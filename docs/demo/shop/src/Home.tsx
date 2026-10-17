import { Offer } from "./Offer";

const OFFERS = [
  {
    name: "The Pair",
    image: "/box-small.webp",
    price: 19,
    serves: "For one or two, about 4 kg",
    contents: ["6 to 8 kinds of the week", "A bunch of fresh herbs", "Recipe card in the box"],
  },
  {
    name: "The Table",
    image: "/box-family.webp",
    price: 32,
    serves: "For a family of four, about 8 kg",
    contents: ["10 to 12 kinds of the week", "Potatoes and onions every time", "Swap two items each week"],
    chosen: true,
  },
  {
    name: "The Chef",
    image: "/box-chef.webp",
    price: 46,
    serves: "For cooks who like a challenge",
    contents: ["Heirloom and rare varieties", "Edible flowers and shoots", "A note from the grower"],
  },
];

const WEEK = [
  ["Monday", "Pick your box, or skip the week. Change it until midnight."],
  ["Tuesday", "Our twelve farms pick at dawn, only what was ordered."],
  ["Thursday", "At your door between 6 and 8 pm, in a crate we take back."],
];

const VOICES = [
  ["I cook more since the crate started coming. Thursdays now have a smell.", "Camille, Croix-Rousse"],
  ["The purple carrots converted my kids. I did not think that was possible.", "Yanis, Villeurbanne"],
  ["You can tell it was in the ground two days ago. Nothing in a shop tastes like that.", "Odile, Écully"],
];

export function Home() {
  return (
    <main>
      <section className="hero">
        <div className="hero-text">
          <h1>This week’s harvest, at your door by&nbsp;Thursday.</h1>
          <p>
            Vegetables from twelve small farms around Lyon, picked on Tuesday and never stored. One box a week,
            skip whenever you like.
          </p>
          <div className="hero-actions">
            <a className="button" href="#boxes">Choose a box</a>
            <a className="quiet" href="#growers">Meet the growers</a>
          </div>
          <p className="facts">Picked within 60 km · No plastic · Crates taken back</p>
        </div>
        <img className="hero-image" src="/hero.webp" alt="A wooden crate of carrots, beetroot and lettuce on a linen table" />
      </section>

      <section className="boxes" id="boxes">
        <div className="section-head">
          <h2>Three boxes, one harvest</h2>
          <p>Every box comes from the same fields. Pick the size of your table, change it any week.</p>
        </div>
        <div className="offers">
          {OFFERS.map((offer) => (
            <Offer key={offer.name} {...offer} />
          ))}
        </div>
      </section>

      <section className="week" id="week">
        <h2>Your week with Verdure</h2>
        <ol>
          {WEEK.map(([day, text]) => (
            <li key={day}>
              <strong>{day}</strong>
              <p>{text}</p>
            </li>
          ))}
        </ol>
      </section>

      <section className="grower" id="growers">
        <img src="/grower.webp" alt="Hélène Marchal kneeling in a row of carrots at dawn" />
        <figure>
          <blockquote>
            “I sow forty kinds of carrots. The box is the only place people get to taste more than one.”
          </blockquote>
          <figcaption>
            Hélène Marchal, Ferme des Saules, Brindas
            <span>Growing for Verdure since 2019</span>
          </figcaption>
        </figure>
      </section>

      <section className="voices">
        {VOICES.map(([quote, who]) => (
          <figure key={who}>
            <blockquote>{quote}</blockquote>
            <figcaption>{who}</figcaption>
          </figure>
        ))}
      </section>

      <footer className="foot">
        <span className="mark">Verdure</span>
        <span>Delivered Thursdays across Lyon and its suburbs</span>
      </footer>
    </main>
  );
}
