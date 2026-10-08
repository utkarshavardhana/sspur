package shop

import (
	"reflect"
	"testing"
)

func TestHidden_eu_rate(t *testing.T) {
	if !(taxRate("EU") == 21 && taxRate("US") == 7 && taxRate("UK") == 20 && taxRate("APAC") == 0) {
		t.Fatal("tax rates")
	}
}

func TestHidden_eu_used(t *testing.T) {
	if !(customerTax(customerNew(1, "a", 10, 100, "EU")) == 210 && invoiceGross(invoiceNew(1, "a", 1, 1000, "EU")) == 1210) {
		t.Fatal("EU rate not used")
	}
}

func TestHidden_money(t *testing.T) {
	if !(money(1005) == "10.05" && money(7) == "0.07" && money(123400) == "1234.00" && money(1999) == "19.99" && money(0) == "0.00") {
		t.Fatal("money")
	}
}

func TestHidden_money_used(t *testing.T) {
	if got := productLine(productNew(4, "pen", 1, 105, "APAC")); got != "pen #4: 1.05" {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_ship_fee(t *testing.T) {
	if !(shipFee(10, false) == 449 && shipFee(10, true) == 898 && shipFee(0, true) == 0) {
		t.Fatal("shipFee")
	}
}

func TestHidden_shipping_callers(t *testing.T) {
	if !(orderShipping(orderNew(1, "o", 10, 1, "US")) == 449 && parcelShipping(parcelNew(2, "p", 4, 1, "US")) == 359 && rmaShipping(rmaNew(3, "r", 1, 1, "UK")) == 314) {
		t.Fatal("shipping callers")
	}
}

func TestHidden_express(t *testing.T) {
	if !(orderExpressShipping(orderNew(1, "o", 10, 1, "US")) == 898 && orderExpressShipping(orderNew(2, "z", 0, 1, "US")) == 0) {
		t.Fatal("express")
	}
}

func TestHidden_restock_all(t *testing.T) {
	ws := []Warehouse{warehouseNew(1, "a", 5, 1, "US"), warehouseDeactivate(warehouseNew(2, "b", 5, 1, "US")), warehouseNew(3, "c", 0, 1, "EU")}
	got := []int{}
	for _, w := range warehouseRestockAll(ws, 4) {
		got = append(got, w.qty)
	}
	if !reflect.DeepEqual(got, []int{9, 5, 4}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_restock_all_keeps(t *testing.T) {
	got := []int{}
	for _, w := range warehouseRestockAll([]Warehouse{warehouseNew(7, "x", 1, 2, "US"), warehouseNew(8, "y", 2, 3, "UK")}, 1) {
		got = append(got, w.id)
	}
	if !reflect.DeepEqual(got, []int{7, 8}) {
		t.Fatalf("got %v", got)
	}
	if n := len(warehouseRestockAll([]Warehouse{}, 3)); n != 0 {
		t.Fatalf("len %d", n)
	}
}

func TestHidden_unchanged(t *testing.T) {
	if !(customerTotal(customerSample()) == 886 && routeTotal(routeSample()) == 1308 && lessonBucket(lessonNew(1, "l", 1, 1, "US")) == "small") {
		t.Fatal("unchanged")
	}
}
