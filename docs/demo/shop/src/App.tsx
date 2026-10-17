import { useEffect, useState } from "react";
import { BASKET, CODES, DELIVERY_FEES, euros, placeOrder } from "./basket";
import { Header } from "./Header";
import { Home } from "./Home";

function Summary() {
  const [code, setCode] = useState("");
  const [percent, setPercent] = useState(0);
  const subtotal = BASKET.reduce((sum, line) => sum + line.price, 0);
  const total = subtotal * (1 - percent / 100);
  const slot = "Evening";
  return (
    <aside className="summary">
      <h2>Summary</h2>
      <dl>
        <dt>Subtotal</dt>
        <dd>{euros(subtotal)}</dd>
        <dt>Delivery</dt>
        <dd>Free</dd>
        <dt className="total">Total</dt>
        <dd className="total">{euros(total)}</dd>
      </dl>
      <p className="when">Charged on Wednesday, once the harvest is confirmed.</p>
      <button className="pay" type="button" onClick={placeOrder}>
        Pay {euros(total + DELIVERY_FEES[slot])} now
      </button>
      <form className="code" onSubmit={(event) => event.preventDefault()}>
        <label htmlFor="code">Gift or promo code</label>
        <input id="code" value={code} onChange={(event) => setCode(event.target.value)} placeholder="WELCOME10" />
        <button type="submit" onClick={() => setPercent(CODES[code.trim().toUpperCase()].percent)}>Apply</button>
      </form>
    </aside>
  );
}

function Checkout() {
  return (
    <main className="checkout">
      <section className="lines">
        <h1>Your basket</h1>
        <ul>
          {BASKET.map((line) => (
            <li key={line.name}>
              <img src={line.image} alt="" />
              <div>
                <strong>{line.name}</strong>
                <span>{line.detail}</span>
              </div>
              <b>{euros(line.price)}</b>
            </li>
          ))}
        </ul>
        <div className="delivery">
          <h2>Delivery</h2>
          <p>Thursday 23 October, between 6 and 8 pm</p>
          <p>14 rue des Capucins, 69001 Lyon</p>
        </div>
      </section>
      <Summary />
    </main>
  );
}

export default function App() {
  useEffect(() => {
    fetch("/_port/3000/api/basket").catch(() => {});
  }, []);
  return (
    <>
      <Header />
      {location.pathname === "/checkout" ? <Checkout /> : <Home />}
    </>
  );
}
