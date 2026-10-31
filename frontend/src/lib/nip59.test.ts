import { describe, expect, it } from "vitest";
import { generateKeypair } from "./nostr";
import { openWrap, wrapShare } from "./nip59";

describe("NIP-59 share wrap", () => {
  it("round-trips a share to the recipient", async () => {
    const sender = generateKeypair();
    const recipient = generateKeypair();
    const wraps = await wrapShare({
      senderSk: sender.secret,
      senderPk: sender.pubkey,
      recipientPk: recipient.pubkey,
      coordination: "aa".repeat(16),
      x: 1,
      bytes: "ab".repeat(32),
    });
    expect(wraps).toHaveLength(2);
    const opened = await openWrap(wraps[0], recipient.secret);
    expect(opened).not.toBeNull();
    expect(opened?.x).toBe(1);
    expect(opened?.bytes).toBe("ab".repeat(32));
    expect(opened?.from).toBe(sender.pubkey);
  });
});
