export function Header() {
  return (
    <header className="top">
      <a className="mark" href="/">
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <path d="M16 29c-3.2-6.4-4.6-12-4.2-16.8h8.4C20.6 17 19.2 22.6 16 29Z" />
          <path d="M16 12c-1-4-3.6-6.6-7.4-7.4M16 12c.4-4.2 2.4-7.4 6-9M16 12c2-2.6 5-3.8 8.6-3.4" />
        </svg>
        Verdure
      </a>
      <nav>
        <a href="/#boxes">Boxes</a>
        <a href="/#week">How it works</a>
        <a href="/#growers">Growers</a>
      </nav>
      <a className="basket-link" href="/checkout">
        Basket <span>3</span>
      </a>
    </header>
  );
}
