export const BASKET = [
  { name: "The Table", detail: "Weekly box, 10 to 12 kinds", image: "/box-family.webp", price: 32 },
  { name: "The Pair, as a gift", detail: "Delivered to Lucie, Saint-Just", image: "/box-small.webp", price: 19 },
  { name: "Heirloom carrots", detail: "Add-on, purple and yellow, 1 kg", image: "/box-chef.webp", price: 6.5 },
];

// Codes the shop accepts. The demo's bug: an unknown code has no entry.
export const CODES: Record<string, { percent: number }> = {
  WELCOME10: { percent: 10 },
};

export const euros = (amount: number) =>
  amount.toLocaleString("fr-FR", { style: "currency", currency: "EUR" });
