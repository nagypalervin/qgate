export function Greeting({ name }: { name?: string }) {
  return <div>{name ? <b>{name}</b> : "anonymous"}</div>;
}
