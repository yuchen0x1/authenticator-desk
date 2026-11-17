use serde::Serialize;
#[derive(Serialize)]
pub struct PlatformTemplate {
    pub issuer: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
    pub help_key: &'static str,
}
pub fn platform_templates() -> Vec<PlatformTemplate> {
    [
        "GitHub",
        "Google",
        "Microsoft",
        "Discord",
        "Cloudflare",
        "Amazon AWS",
        "Steam",
        "Facebook",
        "Instagram",
        "Binance",
        "Custom",
    ]
    .into_iter()
    .map(|issuer| PlatformTemplate {
        issuer,
        icon: match issuer {
            "GitHub" => "github",
            "Google" => "google",
            "Microsoft" => "microsoft",
            "Discord" => "discord",
            "Cloudflare" => "cloud",
            "Amazon AWS" => "aws",
            "Steam" => "steam",
            "Facebook" => "facebook",
            "Instagram" => "instagram",
            "Binance" => "binance",
            _ => "key",
        },
        category: match issuer {
            "GitHub" | "Cloudflare" | "Amazon AWS" => "work",
            "Binance" => "finance",
            _ => "personal",
        },
        help_key: if issuer == "Steam" {
            "template.steam_standard_only"
        } else {
            "template.standard_otp"
        },
    })
    .collect()
}
