export function process(order: Order): number {
  let total = order.base;
  if (order.express) { total += 10; }
  if (order.gift) { total += 5; }
  if (order.weight > 1) { total += 1; }
  if (order.weight > 2) { total += 2; }
  if (order.weight > 4) { total += 4; }
  if (order.weight > 8) { total += 8; }
  if (order.weight > 16) { total += 16; }
  return total;
}
