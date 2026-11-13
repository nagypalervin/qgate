using Shop.Infrastructure.Data;

namespace Shop.Domain;

public class Order
{
    public void Save(Db db) => db.Add(this);
}
