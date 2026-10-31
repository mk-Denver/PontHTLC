import type { Session } from "./session.svelte";

export type StepId = "split" | "invoice" | "fund" | "release" | "close";

export type Step = {
  id: StepId;
  label: string;
  done: boolean;
  current: boolean;
};

export type NextMove = {
  title: string;
  body: string;
  href: string;
};

export function tradeSteps(session: Session, paymentHash: string): Step[] {
  const trade = session.tradeFor(paymentHash);
  const escrow = session.escrows.find((item) => item.payment_hash === paymentHash);
  const shares = session.shareStatus(paymentHash);
  const invoiced = Boolean(trade?.bolt11 || escrow?.bolt11);
  const funded = Boolean(trade?.funded);
  const released = Boolean(trade?.released || shares.held.includes(2) && shares.held.includes(1));
  const closed = trade?.outcome === "settled" || trade?.outcome === "refunded"
    || escrow?.protocol_status === "settled" || escrow?.protocol_status === "refunded";
  const split = Boolean(trade || shares.held.length > 0 || session.offers.some((offer) => offer.paymentHash === paymentHash));

  const flags = [split, invoiced, funded, released, closed];
  let current = flags.findIndex((flag) => !flag);
  if (current === -1) {
    current = flags.length - 1;
  }

  const labels = ["Split P", "Hold invoice", "Fund", "Second share", "Close"];
  const ids: StepId[] = ["split", "invoice", "fund", "release", "close"];
  return ids.map((id, index) => ({
    id,
    label: labels[index],
    done: flags[index],
    current: index === current && !closed,
  }));
}

export function nextMove(session: Session, paymentHash?: string): NextMove | null {
  const hash = paymentHash
    ?? session.selected?.payment_hash
    ?? session.trades[0]?.paymentHash
    ?? session.offers[0]?.paymentHash;
  if (!hash) {
    if (session.role === "seller") {
      return { title: "Start a trade", body: "Originate P and send s1 to the buyer tab.", href: "/app/new" };
    }
    if (session.role === "buyer") {
      return { title: "Wait for an offer", body: "The seller tab sends a hash. Or paste one.", href: "/app/offers" };
    }
    return { title: "Wait for a share", body: "Sellers send you s3 when they name you as arb.", href: "/app/cases" };
  }

  const trade = session.tradeFor(hash);
  const escrow = session.escrows.find((item) => item.payment_hash === hash);
  const shares = session.shareStatus(hash);
  const outcome = trade?.outcome || escrow?.protocol_status;

  if (outcome === "settled") {
    return { title: "This ticket is settled", body: "The HTLC was claimed. Nothing left to do.", href: `/app/trade/${hash}` };
  }
  if (outcome === "refunded" || outcome === "fail_submitted") {
    return { title: "This ticket failed back", body: "Sats returned to the seller.", href: `/app/trade/${hash}` };
  }
  if (outcome === "disputed" && session.role === "arbitrator") {
    return { title: "Release s3", body: "Send the tie-break share to the party you side with.", href: "/app/cases" };
  }

  if (session.role === "seller") {
    if (!trade?.bolt11 && session.invoices.length === 0) {
      return { title: "Wait for the invoice", body: "Buyer creates the hold, then it lands on Fund.", href: "/app/fund" };
    }
    if (!trade?.funded) {
      return { title: "Fund the hold", body: "Pay the BOLT11. The HTLC stays claimable.", href: "/app/fund" };
    }
    if (!trade?.released) {
      return { title: "Release s2", body: "Only after the cash is in your hand.", href: `/app/trade/${hash}` };
    }
    return { title: "Wait for claim", body: "Buyer reconstructs P and claims. Watch the tape.", href: "/app/activity" };
  }

  if (session.role === "buyer") {
    if (!escrow && session.offers.length) {
      return { title: "Hold the invoice", body: "Accept the seller’s hash and publish a BOLT11.", href: "/app/offers" };
    }
    if (escrow && !trade?.funded && !session.events.some((event) => event.kind === "PaymentClaimable")) {
      return { title: "Show the invoice", body: "Seller must fund before you can claim.", href: `/app/escrow/${escrow.id}` };
    }
    if (!shares.ready) {
      return { title: "Wait for a second share", body: "Seller releases s2 — or the arb sends s3.", href: escrow ? `/app/escrow/${escrow.id}` : "/app/offers" };
    }
    if (escrow) {
      return { title: "Reconstruct and claim", body: "P stays in this browser until you press Claim.", href: `/app/escrow/${escrow.id}` };
    }
  }

  if (session.role === "arbitrator") {
    if (shares.held.includes(3)) {
      return { title: "You hold s3", body: "Stay quiet until a dispute. Then send s3 to one side.", href: "/app/cases" };
    }
    return { title: "No share yet", body: "Ask the seller to name your pubkey when they split.", href: "/app/desk" };
  }

  return null;
}
