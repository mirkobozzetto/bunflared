type Props = {
  name: string;
  image: string;
  price: number;
  serves: string;
  contents: string[];
  chosen?: boolean;
};

export function Offer({ name, image, price, serves, contents, chosen }: Props) {
  return (
    <article className={chosen ? "offer chosen" : "offer"}>
      <img src={image} alt="" />
      <div className="offer-head">
        <h3>{name}</h3>
        {chosen && <span className="tag">Most chosen</span>}
      </div>
      <p className="serves">{serves}</p>
      <p className="price">
        {price} € <small>a week</small>
      </p>
      <ul>
        {contents.map((line) => (
          <li key={line}>{line}</li>
        ))}
      </ul>
      <a className="button" href="/checkout">
        Choose {name}
      </a>
    </article>
  );
}
