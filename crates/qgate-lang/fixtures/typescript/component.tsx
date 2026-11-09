// Fixture: TSX. Parsing this as plain TypeScript fails, because `<div>`
// is not a type assertion here.

export function Greeting({ name }: { name?: string }) {
  return <div>{name ? <b>{name}</b> : "anonymous"}</div>;
}

const List = ({ items }: { items: string[] }) => (
  <ul>{items.map((item) => item && <li key={item}>{item}</li>)}</ul>
);
