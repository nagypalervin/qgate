export function process(order: Order): number {
  let total = order.base;
  if (order.express) { total += 10; }
  if (order.gift) { total += 5; }
  return total;
}

export function legacy(kind: number): string {
  switch (kind) {
      case 0: return 'kind 0';
      case 1: return 'kind 1';
      case 2: return 'kind 2';
      case 3: return 'kind 3';
      case 4: return 'kind 4';
      case 5: return 'kind 5';
      case 6: return 'kind 6';
      case 7: return 'kind 7';
  }
  return 'unknown';
}
