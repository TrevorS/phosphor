// T083 auxiliary fixture — TSX, the second language in `tree-sitter-typescript`.
import { useMemo, useState, type ReactNode } from "react";

interface RowProps<T> {
  item: T;
  seen?: boolean;
  children?: ReactNode;
  onPick(item: T): void;
}

export function Row<T extends { id: string }>({ item, seen = false, children, onPick }: RowProps<T>) {
  const [hover, setHover] = useState<boolean>(false);
  const glyph = useMemo(() => (seen ? "" : "●"), [seen]);

  return (
    <li
      className={`row${hover ? " row--hover" : ""}`}
      data-seen={String(seen)}
      onMouseEnter={() => setHover(true)}
      onClick={() => onPick(item)}
    >
      <span aria-hidden>{glyph}</span>
      {children ?? <em>{item.id}</em>}
      <>{seen && <small>seen</small>}</>
    </li>
  );
}

export default function List<T extends { id: string }>({ items }: { items: T[] }) {
  return (
    <ul className="rows">
      {items.map((it) => (
        <Row key={it.id} item={it} onPick={() => void 0} />
      ))}
    </ul>
  );
}
