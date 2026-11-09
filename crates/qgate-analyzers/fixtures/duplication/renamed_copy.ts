// Orders, with a copy of the date helper under other names.
import { Order } from "./order";

export function formatOrderDate(when: Date): string {
  const y = when.getFullYear();
  const m = String(when.getMonth() + 1).padStart(2, "0");
  const dd = String(when.getDate()).padStart(2, "0");
  return `${y}/${m}/${dd}`;
}
