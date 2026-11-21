import type { AccountView, CodeView } from "../../lib/types";
import { AccountCard } from "./AccountCard";

type CardProps = Parameters<typeof AccountCard>[0];
type Props = {
  accounts: AccountView[];
  codes: CodeView[];
  selected: Set<string>;
  masked: boolean;
  viewMode?: string;
  listLabel: string;
  copyAvailable?: boolean;
  advancing?: ReadonlySet<string>;
  favoriting?: ReadonlySet<string>;
  labels: CardProps["t"];
  onSelect: CardProps["onSelect"];
  onEdit: CardProps["onEdit"];
  onFavorite: CardProps["onFavorite"];
  onCopy: CardProps["onCopy"];
  onAdvance: CardProps["onAdvance"];
};

export function AccountList({ accounts, codes, selected, masked, viewMode = "cards", listLabel, copyAvailable = false, advancing, favoriting, labels, onSelect, onEdit, onFavorite, onCopy, onAdvance }: Props) {
  const codesById = new Map(codes.map((code) => [code.account_id, code]));
  return <div className={`account-list ${viewMode === "list" ? "account-list-linear" : ""}`} role="list" aria-label={listLabel}>
    {accounts.map((account) => <AccountCard key={account.id} account={account} code={codesById.get(account.id)} selected={selected.has(account.id)} masked={masked} copyAvailable={copyAvailable} advancing={advancing?.has(account.id)} favoriting={favoriting?.has(account.id)} t={labels} onSelect={onSelect} onEdit={onEdit} onFavorite={onFavorite} onCopy={onCopy} onAdvance={onAdvance} />)}
  </div>;
}
