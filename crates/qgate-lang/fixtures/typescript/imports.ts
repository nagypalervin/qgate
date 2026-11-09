// Fixture: imports that count, and forms that do not.

import { a } from "./money";
import type { T } from "../ui/types";
import "./polyfills";
export * from "../infra/db";
export { b } from "./b";
import fs = require("fs");
import {
  c,
  d,
} from "./multi";

// Not imports: an export without a source, and a local binding.
export const local = 1;
export { a as money };

async function load(name: string) {
  const m = await import("../ui/Modal");
  const x = require("lodash");
  // Not counted: not a single string literal.
  const t = await import(`./locales/${name}`);
  const s = await import(`./static`);
  const i = require(name);
  const two = require("a", "b");
  return [m, x, t, s, i, two];
}

// Type positions count too, entered by the walk or not.
let typed: import("./typed").T;
type Of = typeof import("./typeof");
const asserted = value as import("./asserted").T;

// Module level: these go through the bound-call path of the walk.
const top = require("./top");
const data = await import("./data.json", { with: { type: "json" } });

// Comments between arguments are skipped; an empty specifier is kept.
const commented = require(/* why */ "./commented");
import "";

// Several imports in one skipped type subtree, next to one outside it.
type Pick2<T> = T extends import("./c1").A ? import("./c2").B : never;
let mixed: import("./m1").T = require("./m2");
