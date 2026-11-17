export function process(input: Input): string[] {
  const out: string[] = [];
  if (input.empty) { out.push('empty'); }
  if (input.full) { out.push('full'); }
  if (input.alpha) { out.push('alpha'); }
  if (input.beta) { out.push('beta'); }
  if (input.gamma) { out.push('gamma'); }
  if (input.delta) { out.push('delta'); }
  if (input.epsilon) { out.push('epsilon'); }
  if (input.zeta) { out.push('zeta'); }
  if (input.eta) { out.push('eta'); log(input.eta); }
  if (input.theta) { out.push('theta'); log(input.theta); }
  if (input.iota) { out.push('iota'); log(input.iota); }
  return out;
}
