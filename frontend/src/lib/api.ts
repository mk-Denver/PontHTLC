export type EscrowRecord = {
  id: string;
  owner: string;
  payment_hash: string;
  state: string;
  bolt11: string;
  protocol_status: string;
};

export type WireEvent = {
  kind: string;
  payment_hash: string | null;
};

export type NodeInfo = {
  node_pubkey: string;
  block_height: number;
};

export type CreateResponse = {
  id: string;
  payment_hash: string;
  bolt11_invoice: string;
};

export type PayResponse = {
  payment_hash: string;
};

export type NostrEventJson = {
  id: string;
  pubkey: string;
  created_at: number;
  kind: number;
  tags: string[][];
  content: string;
  sig: string;
};

export type DisputeResponse = {
  gift_wraps: NostrEventJson[];
};

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

export function apiBase(): string {
  return localStorage.getItem("ponthtlc.api") ?? "";
}

async function request<T>(
  pubkey: string,
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set("X-Nostr-Pubkey", pubkey);
  if (init.body !== undefined && !headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  const response = await fetch(`${apiBase()}${path}`, { ...init, headers });
  const text = await response.text();
  if (!response.ok) {
    throw new ApiError(response.status, text || response.statusText);
  }
  if (!text) {
    return undefined as T;
  }
  return JSON.parse(text) as T;
}

export function health(): Promise<boolean> {
  return fetch(`${apiBase()}/health/live`)
    .then((response) => response.ok)
    .catch(() => false);
}

export function createEscrow(
  pubkey: string,
  paymentHash: string,
  amountMsat: number,
): Promise<CreateResponse> {
  return request(pubkey, "/api/escrow/create", {
    method: "POST",
    body: JSON.stringify({ payment_hash: paymentHash, amount_msat: amountMsat }),
  });
}

export function listEscrows(pubkey: string): Promise<EscrowRecord[]> {
  return request(pubkey, "/api/escrows");
}

export function getEscrow(pubkey: string, id: string): Promise<EscrowRecord> {
  return request(pubkey, `/api/escrow/${id}`);
}

export function fundEscrow(pubkey: string, id: string, bolt11: string): Promise<PayResponse> {
  return request(pubkey, `/api/escrow/${id}/fund`, {
    method: "POST",
    body: JSON.stringify({ bolt11 }),
  });
}

export function payInvoice(pubkey: string, bolt11: string): Promise<PayResponse> {
  return request(pubkey, "/api/invoice/pay", {
    method: "POST",
    body: JSON.stringify({ bolt11 }),
  });
}

export function claimEscrow(pubkey: string, id: string, preimage: string): Promise<void> {
  return request(pubkey, `/api/escrow/${id}/claim`, {
    method: "POST",
    body: JSON.stringify({ preimage }),
  });
}

export function failEscrow(pubkey: string, id: string): Promise<void> {
  return request(pubkey, `/api/escrow/${id}/fail`, { method: "POST", body: "{}" });
}

export function disputeEscrow(
  pubkey: string,
  id: string,
  resolver: string,
): Promise<DisputeResponse> {
  return request(pubkey, `/api/escrow/${id}/dispute`, {
    method: "POST",
    body: JSON.stringify({ resolver }),
  });
}

export function nodeInfo(pubkey: string): Promise<NodeInfo> {
  return request(pubkey, "/api/node/info");
}

export function nodeChannels(pubkey: string): Promise<string[]> {
  return request(pubkey, "/api/node/channels");
}

export function openEventStream(
  pubkey: string,
  onEvent: (event: WireEvent) => void,
): () => void {
  const url = new URL(`${apiBase() || window.location.origin}/api/ws`);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  url.searchParams.set("pubkey", pubkey);
  const socket = new WebSocket(url);
  socket.onmessage = (message) => {
    try {
      onEvent(JSON.parse(String(message.data)) as WireEvent);
    } catch {
      return;
    }
  };
  return () => socket.close();
}
