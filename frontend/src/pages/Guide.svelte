<h1>How a trade moves</h1>
<p class="tiny">Three tabs. One hold invoice. Nobody reconstructs P except the winner’s browser.</p>

<div class="grid" style="margin-top:18px">
  <article class="panel">
    <div class="kicker">Seller</div>
    <h2>Kwame has bitcoin</h2>
    <ol class="tiny" style="padding-left:18px;line-height:1.9">
      <li>Sign up as seller. Open the buyer tab beside you.</li>
      <li>On Desk, open the internal channel and mine a few blocks.</li>
      <li>New trade: amount, buyer key, optional arb key.</li>
      <li>s1 goes to the buyer. s3 goes to the arb. You keep s2.</li>
      <li>When the hold invoice arrives, fund it.</li>
      <li>After cash is in your hand, release s2 — or wait out a dispute.</li>
    </ol>
  </article>
  <article class="panel">
    <div class="kicker">Buyer</div>
    <h2>Amina has cash</h2>
    <ol class="tiny" style="padding-left:18px;line-height:1.9">
      <li>Sign up as buyer. Offers appear when the seller splits.</li>
      <li>Hold the invoice. Show the QR if they are not on this browser.</li>
      <li>Wait for PaymentClaimable. That means the sats are locked.</li>
      <li>Hand over cash. Seller releases s2 (or arb sends s3).</li>
      <li>Reconstruct P here, then Claim. Or Fail. Or Dispute.</li>
    </ol>
  </article>
  <article class="panel">
    <div class="kicker">Arbitrator</div>
    <h2>Fatou holds the tie-break</h2>
    <ol class="tiny" style="padding-left:18px;line-height:1.9">
      <li>Sign up as arbitrator. Give your pubkey to the seller.</li>
      <li>You receive s3 at split time. You cannot settle alone.</li>
      <li>On dispute, read the street story off-band.</li>
      <li>Send s3 to the buyer to let them claim, or to the seller to keep them even.</li>
      <li>You never talk to the hosted node.</li>
    </ol>
  </article>
</div>
