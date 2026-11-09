// Fixture: normalized tokens.
import { money } from "./money";
export * from "./all";
export { b } from "./b";
interface Shape {
  size: number;
}
type Id = string;

export function fmt(d: Date): string {
  // A comment produces no token.
  const y = d.getFullYear();
  return `${y}-${'x'}` + 1;
}
