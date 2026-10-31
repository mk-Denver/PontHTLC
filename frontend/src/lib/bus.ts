import type { NostrEvent } from "./nostr";

export type Role = "buyer" | "seller" | "arbitrator";

export type HelloMessage = {
  type: "hello";
  role: Role;
  pubkey: string;
  name: string;
};

export type OfferMessage = {
  type: "offer";
  paymentHash: string;
  amountMsat: number;
  seller: string;
};

export type InvoiceMessage = {
  type: "invoice";
  escrowId: string;
  bolt11: string;
  paymentHash: string;
  buyer: string;
};

export type WrapMessage = {
  type: "wrap";
  event: NostrEvent;
};

export type DisputeMessage = {
  type: "dispute";
  paymentHash: string;
  escrowId: string;
  from: string;
  resolver: string;
};

export type BusMessage = HelloMessage | OfferMessage | InvoiceMessage | WrapMessage | DisputeMessage;

const CHANNEL = "ponthtlc-share";

export function openBus(onMessage: (message: BusMessage) => void): {
  send: (message: BusMessage) => void;
  close: () => void;
} {
  const channel = new BroadcastChannel(CHANNEL);
  channel.onmessage = (event) => {
    const message = event.data as BusMessage;
    if (!message || typeof message !== "object" || !("type" in message)) {
      return;
    }
    onMessage(message);
  };
  return {
    send: (message) => channel.postMessage(message),
    close: () => channel.close(),
  };
}

export function openRelay(
  url: string,
  pubkey: string,
  onWrap: (event: NostrEvent) => void,
): { publish: (event: NostrEvent) => void; close: () => void } {
  const socket = new WebSocket(url);
  socket.onopen = () => {
    socket.send(JSON.stringify(["REQ", "ponthtlc", { kinds: [1059], "#p": [pubkey] }]));
  };
  socket.onmessage = (event) => {
    try {
      const parsed = JSON.parse(String(event.data)) as unknown[];
      if (parsed[0] === "EVENT" && parsed[2] && typeof parsed[2] === "object") {
        onWrap(parsed[2] as NostrEvent);
      }
    } catch {
      return;
    }
  };
  return {
    publish: (nostrEvent) => {
      if (socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify(["EVENT", nostrEvent]));
      }
    },
    close: () => {
      if (socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify(["CLOSE", "ponthtlc"]));
      }
      socket.close();
    },
  };
}
