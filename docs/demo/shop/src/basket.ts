export const BASKET = [
  { name: "The Table", detail: "Weekly box, 10 to 12 kinds", image: "/box-family.webp", price: 32 },
  { name: "The Pair, as a gift", detail: "Delivered to Lucie, Saint-Just", image: "/box-small.webp", price: 19 },
  { name: "Heirloom carrots", detail: "Add-on, purple and yellow, 1 kg", image: "/box-chef.webp", price: 6.5 },
];

// The demo's bugs: the slot is spelled "Evening" where it is used, so the pay
// button adds undefined and says NaN; an unknown code has no entry.
export const DELIVERY_FEES: Record<string, number> = { evening: 0, late: 2.5 };

// Codes the shop accepts.
export const CODES: Record<string, { percent: number }> = {
  WELCOME10: { percent: 10 },
};

export const euros = (amount: number) =>
  amount.toLocaleString("fr-FR", { style: "currency", currency: "EUR" });

export const placeOrder = () =>
  fetch("/_port/3000/api/orders", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ lines: BASKET.map((line) => line.name), slot: "Thursday 18:00-20:00" }),
  })
    .then((response) => response.ok)
    .catch(() => false);
