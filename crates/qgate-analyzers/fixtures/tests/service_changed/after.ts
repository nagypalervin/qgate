// Fixture: a service whose method bodies the change edits.
// Asserted in src/tests.rs.
import { Injectable } from "@angular/core";

import { Item } from "./item";

@Injectable({ providedIn: "root" })
export class CartService {
  private items: Item[] = [];

  total(): number {
    return this.items.reduce((sum, item) => sum + item.price * item.quantity, 0);
  }

  add(item: Item): void {
    if (!this.items.includes(item)) this.items.push(item);
  }

  remove(item: Item): void {
    this.items = this.items.filter((i) => i.id !== item.id);
  }

  clear(): void {
    this.items = [];
  }
}
