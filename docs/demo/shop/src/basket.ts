export const BASKET = [
  { name: "The Table", detail: "Weekly box, 10 to 12 kinds", image: "/box-family.webp", price: 32 },
  { name: "Free-range eggs", detail: "Six, from Ferme des Saules", image: "/box-small.webp", price: 4.8 },
  { name: "Country sourdough", detail: "1 kg, Fournil de Brindas", image: "/box-chef.webp", price: 6.2 },
];

// Codes the shop accepts. The demo's bug: an unknown code has no entry.
export const CODES: Record<string, { percent: number }> = {
  WELCOME10: { percent: 10 },
};

export const euros = (amount: number) =>
  amount.toLocaleString("en-GB", { style: "currency", currency: "EUR" });
