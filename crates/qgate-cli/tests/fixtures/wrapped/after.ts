// A Next.js-style route handler wrapped in a call at module level.
export const GET = withAuth(async (req: Request) => {
  if (!req.ok) {
    return fail();
  }
  if (req.cached && req.fresh) {
    return cached();
  }
  return ok();
});
