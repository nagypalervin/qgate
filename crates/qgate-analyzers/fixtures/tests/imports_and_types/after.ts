// Fixture: a service whose method bodies the change edits.
// Asserted in src/tests.rs.
import { Item } from "./item";

import { Injectable } from "@angular/core";

export interface CartSummary {
  count: number;
  total: number;
}

@Injectable({ providedIn: "root" })
export class CartService {
  private items: Item[] = [];

  total(): number {
    return this.items.reduce((sum, item) => sum + item.price, 0);
  }

  add(item: Item): void {
    this.items.push(item);
  }

  remove(item: Item): void {
    this.items = this.items.filter((i) => i !== item);
  }

  clear(): void {
    this.items = [];
  }
}
