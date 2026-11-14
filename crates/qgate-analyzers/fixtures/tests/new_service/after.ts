// Fixture: a new service, added by the change without a spec.
// Asserted in src/tests.rs.
import { HttpClient } from "@angular/common/http";
import { inject } from "@angular/core";
import { Order } from "./order";

export class OrderService {
  load(id: string) {
    return this.http.get<Order>(`/api/orders/${id}`);
  }

  save(order: Order) {
    return this.http.put(`/api/orders/${order.id}`, order);
  }

  private readonly http = inject(HttpClient);
}
