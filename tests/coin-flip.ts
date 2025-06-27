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

  it("init tressay", async () => {
    console.clear();

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

  let testContext: {
    keypair: any;
    connection: any;
    program: any;
    sbProgram: any;
    queue: any;
    randomness: any;
    id: number;
  };

  it("create randomness account", async () => {
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

    testContext = {
      keypair,
      connection,
      program,
      sbProgram,
      queue,
      randomness,
      id,
    };
  });

  it("commit randomness and take funds from the user", async () => {
    if (!testContext) {
      throw new Error("Init test must run before reveal test");
    }

    const { randomness, id, keypair, connection, queue, sbProgram } =
      testContext;

    const commitIx = await randomness.commitIx(queue);

    const coinFlipIx = await myProgram.methods
      .diceRoll(new BN(id), 50, new BN(0.1 * LAMPORTS_PER_SOL), { over: {} })
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
  });

  it("reveal dice rool", async () => {
    if (!testContext) {
      throw new Error("Init test must run before reveal test");
    }

    const { randomness, id, keypair, connection, sbProgram } = testContext;

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
    console.log("Transaction Signature revealTx", sig5);

    const txlogs = await connection.getParsedTransaction(sig5, {
      maxSupportedTransactionVersion: 0,
    });

    console.log(txlogs?.meta?.logMessages);
  });

  it("invalid guess", async () => {
    const { keypair } = testContext;
    try {
      await myProgram.methods
        .diceRoll(new BN(12321), 200, new BN(0.1 * LAMPORTS_PER_SOL), {
          over: {},
        })
        .accounts({
          user: keypair.publicKey,
          randomnessAccountData: rngKp.publicKey,
        })
        .rpc();
    } catch (error) {
      assert(error.errorLogs[0].includes("InvalidGuess"));
    }
  });

  it("invalid bet amount", async () => {
    const { keypair } = testContext;
    try {
      await myProgram.methods
        .diceRoll(new BN(12321), 20, new BN(10 * LAMPORTS_PER_SOL), {
          over: {},
        })
        .accounts({
          user: keypair.publicKey,
          randomnessAccountData: rngKp.publicKey,
        })
        .rpc();
    } catch (error) {
      assert(error.errorLogs[0].includes("BetOutOfRange"));
    }
  });
});
