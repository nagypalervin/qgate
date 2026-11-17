export function handle(input: Input): string[] {
  const out: string[] = [];
  if (input.alpha) { out.push('alpha'); }
  if (input.beta) { out.push('beta'); }
  return out;
}

export function helper(input: Input): string[] {
  const out: string[] = [];
  if (input.alpha) { out.push('alpha'); }
  if (input.beta) { out.push('beta'); }
  return out;
}
