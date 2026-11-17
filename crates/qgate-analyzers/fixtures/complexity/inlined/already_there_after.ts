export function handle(input: Input): string[] {
  const out: string[] = [];
  if (input.alpha) { out.push('alpha'); }
  if (input.beta) { out.push('beta'); }
  if (input.size > 10) { log(input.size); }
  return out;
}
