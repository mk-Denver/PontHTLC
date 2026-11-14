import { bytesToHex } from "./hex";
import { generateKeypair } from "./nostr";

const KEY = "ponthtlc.wallet";

type Saved = {
  pubkey: string;
  name: string;
  lightningAddress: string;
  isAgent: boolean;
};

function fresh(): { secret: string; pubkey: string; name: string; lightningAddress: string; isAgent: boolean } {
  const keys = generateKeypair();
  return {
    secret: bytesToHex(keys.secret),
    pubkey: keys.pubkey,
    name: "",
    lightningAddress: "",
    isAgent: false,
  };
}

function load(): { secret: string; pubkey: string; name: string; lightningAddress: string; isAgent: boolean } {
  try {
    const raw = sessionStorage.getItem(KEY);
    if (raw) {
      const saved = JSON.parse(raw) as Saved;
      if (saved.pubkey) {
        return {
          secret: "",
          pubkey: saved.pubkey,
          name: saved.name ?? "",
          lightningAddress: saved.lightningAddress ?? "",
          isAgent: saved.isAgent ?? false,
        };
      }
    }
  } catch {
  }
  const saved = fresh();
  sessionStorage.setItem(KEY, JSON.stringify({ pubkey: saved.pubkey, name: saved.name, lightningAddress: saved.lightningAddress, isAgent: saved.isAgent }));
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
