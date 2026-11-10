import { bytesToHex } from "./hex";
import { generateKeypair } from "./nostr";

const KEY = "ponthtlc.wallet";

type Saved = {
  secret: string;
  pubkey: string;
  name: string;
  lightningAddress: string;
  isAgent: boolean;
};

function fresh(): Saved {
  const keys = generateKeypair();
  return {
    secret: bytesToHex(keys.secret),
    pubkey: keys.pubkey,
    name: "",
    lightningAddress: "",
    isAgent: false,
  };
}

function load(): Saved {
  try {
    const raw = sessionStorage.getItem(KEY);
    if (raw) {
      const saved = JSON.parse(raw) as Saved;
      if (saved.secret && saved.pubkey) {
        return saved;
      }
    }
  } catch {
    /* a broken saved wallet starts over */
  }
  const saved = fresh();
  sessionStorage.setItem(KEY, JSON.stringify(saved));
  return saved;
}

export class WalletSession {
  secret = $state("");
  pubkey = $state("");
  name = $state("");
  lightningAddress = $state("");
  isAgent = $state(false);
  ready = $state(false);

  boot() {
    const saved = load();
    this.secret = saved.secret;
    this.pubkey = saved.pubkey;
    this.name = saved.name;
    this.lightningAddress = saved.lightningAddress;
    this.isAgent = saved.isAgent;
    this.ready = true;
  }

  save() {
    sessionStorage.setItem(
      KEY,
      JSON.stringify({
        secret: this.secret,
        pubkey: this.pubkey,
        name: this.name,
        lightningAddress: this.lightningAddress,
        isAgent: this.isAgent,
      }),
    );
  }

  toggleAgent() {
    this.isAgent = !this.isAgent;
    this.save();
  }
}

export const wallet = new WalletSession();
