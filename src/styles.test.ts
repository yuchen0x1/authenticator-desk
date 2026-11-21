import { describe, expect, it } from "vitest";
import stylesheet from "./styles.css?raw";

function declarations(selector: string) {
  const start = stylesheet.indexOf(selector);
  if (start < 0) throw new Error(`Missing token block: ${selector}`);
  const body = stylesheet.slice(start, stylesheet.indexOf("}", start) + 1);
  return { body, tokens: Object.fromEntries([...body.matchAll(/(--[\w-]+):\s*([^;]+);/g)].map((match) => [match[1], match[2].trim()])) };
}
const dark = declarations(":root {");
const light = declarations(':root[data-theme="light"]');
const themes = { dark: dark.tokens, light: { ...dark.tokens, ...light.tokens } };
type Color = [number, number, number];
function rgb(hex: string): Color {
  if (!/^#[\da-f]{6}$/i.test(hex)) throw new Error(`Expected six-digit color, got ${hex}`);
  return [1, 3, 5].map((offset) => Number.parseInt(hex.slice(offset, offset + 2), 16)) as Color;
}
function luminance(color: Color) {
  const [r, g, b] = color.map((value) => {
    const channel = value / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function contrast(a: Color, b: Color) {
  const values = [luminance(a), luminance(b)];
  return (Math.max(...values) + 0.05) / (Math.min(...values) + 0.05);
}
function composite(foreground: Color, background: Color, alpha: number): Color {
  return foreground.map((channel, i) => channel * alpha + background[i] * (1 - alpha)) as Color;
}

describe("dual-theme semantic token contract", () => {
  it("maps every theme-dependent color and shadow in both themes", () => {
    const themed = Object.entries(dark.tokens).filter(([, value]) => /#|rgb\(/.test(value)).map(([key]) => key);
    expect(Object.keys(light.tokens).sort()).toEqual(themed.sort());
    expect(dark.body).toContain("color-scheme: dark");
    expect(light.body).toContain("color-scheme: light");
  });

  it("has no missing or self-referencing variables and no component color literals", () => {
    for (const match of stylesheet.matchAll(/var\((--[\w-]+)\)/g)) expect(dark.tokens, match[1]).toHaveProperty(match[1]);
    for (const tokens of Object.values(themes)) for (const [key, value] of Object.entries(tokens)) expect(value, key).not.toContain(`var(${key})`);
    const components = stylesheet.replace(dark.body, "").replace(light.body, "");
    expect(components).not.toMatch(/#[\da-f]{3,8}\b|\brgba?\s*\(/i);
    expect(components).toContain("background:var(--scrim)");
    expect(components).toContain("input::placeholder,textarea::placeholder { color:var(--muted); opacity:1; }");
    expect(components).toContain("scrollbar-color:var(--scroll-thumb) var(--scroll-track)");
  });

  for (const [name, tokens] of Object.entries(themes)) describe(name, () => {
    for (const foreground of ["--text", "--muted", "--accent", "--danger", "--warn-text"]) {
      it(`${foreground} meets AA body-text contrast on each content layer`, () => {
        for (const surface of ["--canvas", "--surface", "--surface-raised", "--control"]) {
          expect(contrast(rgb(tokens[foreground]), rgb(tokens[surface])), `${foreground} / ${surface}`).toBeGreaterThanOrEqual(4.5);
        }
      });
    }
    it("keeps white primary-button text readable in default and hover states", () => {
      for (const fill of ["--accent-fill", "--accent-fill-hover"]) expect(contrast(rgb(tokens["--on-accent"]), rgb(tokens[fill]))).toBeGreaterThanOrEqual(4.5);
    });
    it("keeps input boundaries, focus indicators and scrollbar thumbs distinguishable", () => {
      for (const surface of ["--canvas", "--surface", "--surface-raised"]) {
        for (const line of ["--control-line", "--accent", "--accent-strong", "--danger-line"]) expect(contrast(rgb(tokens[line]), rgb(tokens[surface])), `${line} / ${surface}`).toBeGreaterThanOrEqual(3);
      }
      expect(contrast(rgb(tokens["--scroll-thumb"]), rgb(tokens["--scroll-track"]))).toBeGreaterThanOrEqual(3);
    });
    it("keeps the pressed segment labelled and outlined against the control track", () => {
      // The selected tab is marked by an inset `--accent` ring plus heavier text,
      // so both the ring and the label must clear the thresholds on their own.
      expect(contrast(rgb(tokens["--accent"]), rgb(tokens["--control"]))).toBeGreaterThanOrEqual(3);
      expect(contrast(rgb(tokens["--text-strong"]), rgb(tokens["--surface-raised"]))).toBeGreaterThanOrEqual(4.5);
    });
    it("keeps warning and import-state text readable over their translucent fills", () => {
      const warning = /rgb\((\d+) (\d+) (\d+) \/ (\d+)%\)/.exec(tokens["--warn-soft"]);
      expect(warning).not.toBeNull();
      for (const surface of ["--canvas", "--surface"]) {
        const background = rgb(tokens[surface]);
        const fill = composite([Number(warning![1]), Number(warning![2]), Number(warning![3])], background, Number(warning![4]) / 100);
        expect(contrast(rgb(tokens["--warn-text"]), fill)).toBeGreaterThanOrEqual(4.5);
        for (const color of ["--teal", "--amber", "--danger"]) expect(contrast(rgb(tokens[color]), composite(rgb(tokens[color]), background, 0.14))).toBeGreaterThanOrEqual(4.5);
      }
    });
  });
});
