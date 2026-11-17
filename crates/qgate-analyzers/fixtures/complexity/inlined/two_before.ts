export function a(input: Input): string[] {
  const out: string[] = [];
  if (input.empty) { return out; }
  return out;
}

export function b(input: Input): string[] {
  const out: string[] = [];
  if (input.empty) { return out; }
  return out;
}

export function helper(input: Input): string[] {
  const out: string[] = [];
  if (input.flag0) { out.push('flag 0'); }
  if (input.flag1) { out.push('flag 1'); }
  if (input.flag2) { out.push('flag 2'); }
  if (input.flag3) { out.push('flag 3'); }
  if (input.flag4) { out.push('flag 4'); }
  return out;
}
