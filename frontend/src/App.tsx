import type { ReactElement } from "react";

export function App(): ReactElement {
  return (
    <main className="app-shell">
      <section className="hero">
        <p className="eyebrow">GraphBench</p>
        <h1>Graph-first benchmark surfaces for evidence, context, and replay.</h1>
        <p className="lede">
          The frontend is intentionally thin for now. It exists to enforce a strict
          React plus TypeScript plus Vite boundary while backend schemas and traces come online.
        </p>
      </section>
    </main>
  );
}
