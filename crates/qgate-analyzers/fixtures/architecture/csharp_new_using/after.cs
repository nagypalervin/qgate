using System;
using Shop.Infrastructure.Data;

namespace Shop.Domain;

public class Order
{
    public decimal Total { get; set; }

    public void Save(Db db) => db.Add(this);
}
