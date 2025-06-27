import * as anchor from "@coral-xyz/anchor";
import * as sb from "@switchboard-xyz/on-demand";
import { Program } from "@coral-xyz/anchor";
import { CoinFlip } from "../target/types/coin_flip";
import { loadSbProgram, setupQueue } from "./utilts";
import { Keypair, LAMPORTS_PER_SOL, PublicKey } from "@solana/web3.js";
import { BN } from "bn.js";
import { assert } from "chai";

describe("coin-flip", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const myProgram = anchor.workspace.CoinFlip as Program<CoinFlip>;

  const rngKp = Keypair.generate();

  console.clear();

  it("init tressay", async () => {
    const { keypair } = await sb.AnchorUtils.loadEnv();

    const [address, bump] = PublicKey.findProgramAddressSync(
      [Buffer.from("treasury")],
      myProgram.programId
    );

    // await myProgram.methods
    //   .initializeTreasury()
    //   .accounts({ signer: keypair.publicKey })
    //   .signers([keypair])
    //   .rpc();

    const data = await myProgram.account.treasury.fetch(new PublicKey(address));

    assert.ok(data.owner.equals(keypair.publicKey));
    assert.ok(data.bump === bump);
  });

  it("roll the dice", async () => {
    const { keypair, connection, program } = await sb.AnchorUtils.loadEnv();
    const sbProgram = await loadSbProgram(program!.provider);
    let queue = await setupQueue(program);

    const [randomness, ix] = await sb.Randomness.create(
      sbProgram,
      rngKp,
      queue
    );

    console.log("Randomness account", randomness.pubkey.toString());

    const createRandomnessTx = await sb.asV0Tx({
      connection: sbProgram.provider.connection,
      ixs: [ix],
      payer: keypair.publicKey,
      signers: [keypair, rngKp],
      computeUnitPrice: 75_000,
      computeUnitLimitMultiple: 1.3,
    });

    await connection.simulateTransaction(createRandomnessTx);
    const sig1 = await connection.sendTransaction(createRandomnessTx);
    await connection.confirmTransaction(sig1, "confirmed");
    console.log(" randomness account creation: ", sig1);

    const id = Math.floor(Math.random() * 1000000000);
    const commitIx = await randomness.commitIx(queue);

    const coinFlipIx = await myProgram.methods
      .coinFlip(new BN(id), 50, new BN(0.1 * LAMPORTS_PER_SOL))
      .accounts({
        user: keypair.publicKey,
        randomnessAccountData: rngKp.publicKey,
      })
      .instruction();

    const commitTx = await sb.asV0Tx({
      connection: sbProgram.provider.connection,
      ixs: [commitIx, coinFlipIx],
      payer: keypair.publicKey,
      signers: [keypair],
      computeUnitPrice: 75_000,
      computeUnitLimitMultiple: 1.3,
    });

    await connection.simulateTransaction(commitTx);
    const sig4 = await connection.sendTransaction(commitTx);
    await connection.confirmTransaction(sig4);
    console.log("Transaction Signature commitTx", sig4);

    const revealIx = await randomness.revealIx();

    const settleFlipIx = await myProgram.methods
      .settleFlip(new BN(id))
      .accounts({
        user: keypair.publicKey,
        randomnessAccountData: rngKp.publicKey,
      })
      .instruction();

    const revealTx = await sb.asV0Tx({
      connection: sbProgram.provider.connection,
      ixs: [revealIx, settleFlipIx],
      payer: keypair.publicKey,
      signers: [keypair],
      computeUnitPrice: 75_000,
      computeUnitLimitMultiple: 1.3,
    });

    await connection.simulateTransaction(revealTx);
    const sig5 = await connection.sendTransaction(revealTx);
    await connection.confirmTransaction(sig5);
    console.log("  Transaction Signature revealTx", sig5);

    const answer = await connection.getParsedTransaction(sig5, {
      maxSupportedTransactionVersion: 0,
    });
    let resultLog = answer?.meta?.logMessages?.filter((line) =>
      line.includes("FLIP_RESULT")
    )[0];
    let result = resultLog?.split(": ")[2];

    console.log("\nYour guess is ", true ? "Heads" : "Tails");

    console.log(`\nAnd the random result is ... ${result}!`);
  });
});
