import * as anchor from "@project-serum/anchor";
import { Program, BN } from "@project-serum/anchor";
import { Mux } from "../target/types/mux";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { assert } from "chai";

const SOL1 = 1000000000;

const logTx = async (provider, tx) => {
  console.log(
    (await provider.connection.getConfirmedTransaction(tx, "confirmed")).meta
      .logMessages
  );
};

const getRandomAmount = (min, range) => {
  return min + Math.floor(Math.random() * range);
};

const donate = async (program, benefactor, fundWalletKey, amount) => {
  await program.provider.send(
    new anchor.web3.Transaction().add(
      anchor.web3.SystemProgram.transfer({
        fromPubkey: benefactor.publicKey,
        toPubkey: fundWalletKey,
        lamports: amount,
      })
    ),
    [benefactor],
    { commitment: "confirmed" }
  );
};

const createFund = async (program, creator, verbose = false) => {
  let [walletStateKey, _walletStateBump] = await PublicKey.findProgramAddress(
    [creator.publicKey.toBuffer()],
    program.programId
  );
  let [fundWalletKey, _fundWalletBump] = await PublicKey.findProgramAddress(
    [walletStateKey.toBuffer()],
    program.programId
  );
  let [creatorStateKey, _creatorStateBump] = await PublicKey.findProgramAddress(
    [walletStateKey.toBuffer(), creator.publicKey.toBuffer()],
    program.programId
  );
  const tx = await program.rpc.createFund(new BN(10000), {
    accounts: {
      walletState: walletStateKey,
      fundWallet: fundWalletKey,
      creator: creator.publicKey,
      creatorState: creatorStateKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    },
    signers: [creator],
  });
  await program.provider.connection.confirmTransaction(tx, "confirmed");
  if (verbose) await logTx(program.provider, tx);
  return [walletStateKey, fundWalletKey, creatorStateKey];
};

const createAndTransferStake = async (
  program,
  walletStateKey,
  fundWalletKey,
  recipient,
  sender,
  senderStateKey,
  numShares,
  verbose = false
) => {
  let [recipientStateKey, _recipientStateBump] =
    await PublicKey.findProgramAddress(
      [walletStateKey.toBuffer(), recipient.publicKey.toBuffer()],
      program.programId
    );
  let createStakeAccountTx = await program.transaction.createStakeAccount({
    accounts: {
      walletState: walletStateKey,
      payer: sender.publicKey,
      user: recipient.publicKey,
      userState: recipientStateKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    },
    signers: [sender],
  });
  let transferTx = await program.transaction.transferShares(new BN(numShares), {
    accounts: {
      walletState: walletStateKey,
      fundWallet: fundWalletKey,
      sender: sender.publicKey,
      senderState: senderStateKey,
      recipientState: recipientStateKey,
      recipient: recipient.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    },
    signers: [sender],
  });
  const tx = await program.provider.send(
    new anchor.web3.Transaction().add(createStakeAccountTx).add(transferTx),
    [sender]
  );
  await program.provider.connection.confirmTransaction(tx, "confirmed");
  if (verbose) await logTx(program.provider, tx);
};

const transferStake = async (
  program,
  walletStateKey,
  fundWalletKey,
  recipient,
  sender,
  senderStateKey,
  numShares,
  verbose = false
) => {
  let [recipientStateKey, _recipientStateBump] =
    await PublicKey.findProgramAddress(
      [walletStateKey.toBuffer(), recipient.publicKey.toBuffer()],
      program.programId
    );
  const tx = await program.rpc.transferShares(new BN(numShares), {
    accounts: {
      walletState: walletStateKey,
      fundWallet: fundWalletKey,
      sender: sender.publicKey,
      senderState: senderStateKey,
      recipientState: recipientStateKey,
      recipient: recipient.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    },
    signers: [sender],
  });
  await program.provider.connection.confirmTransaction(tx, "confirmed");
  if (verbose) await logTx(program.provider, tx);
};

const claim = async (
  program,
  walletStateKey,
  fundWalletKey,
  recipient,
  verbose = false
) => {
  let [recipientStateKey, _recipientStateBump] =
    await PublicKey.findProgramAddress(
      [walletStateKey.toBuffer(), recipient.publicKey.toBuffer()],
      program.programId
    );

  const tx = await program.rpc.claim({
    accounts: {
      walletState: walletStateKey,
      fundWallet: fundWalletKey,
      recipient: recipient.publicKey,
      recipientState: recipientStateKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    },
  });
  await program.provider.connection.confirmTransaction(tx, "confirmed");
  if (verbose) await logTx(program.provider, tx);
};

describe("mux", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.Mux as Program<Mux>;
  const creator = Keypair.generate();
  const benefactor = Keypair.generate();
  let daoMembers: Keypair[] = [];
  for (let i = 0; i < 50; ++i) {
    daoMembers.push(Keypair.generate());
  }
  it("Initialize start state", async () => {
    // Airdropping tokens to a payer.
    await program.provider.connection.confirmTransaction(
      await program.provider.connection.requestAirdrop(
        benefactor.publicKey,
        100000000000000
      ),
      "confirmed"
    );
    await program.provider.connection.confirmTransaction(
      await program.provider.connection.requestAirdrop(
        creator.publicKey,
        10000000000
      ),
      "confirmed"
    );
  });
  it("Test Fund", async () => {
    let walletState;
    console.log("Creating wallet");
    const [walletStateKey, fundWalletKey, creatorStateKey] = await createFund(
      program,
      creator,
      true
    );
    let totalSent = 0;
    console.log("Seeding inital stake");
    for (let i = 0; i < 10; ++i) {
      let amount = getRandomAmount(SOL1, SOL1);
      totalSent += amount;
      await donate(program, benefactor, fundWalletKey, amount);
      const recipient = daoMembers[i];
      let bps = 50 * (i + 1);
      console.log(`Sending ${bps} bps to ${recipient.publicKey.toBase58()}`);
      await createAndTransferStake(
        program,
        walletStateKey,
        fundWalletKey,
        recipient,
        creator,
        creatorStateKey,
        bps
      );
    }

    walletState = await program.account.walletState.fetch(
      walletStateKey,
      "confirmed"
    );

    let dust = walletState.totalShares.toNumber() - walletState.dust.toNumber();
    totalSent += dust;
    await donate(program, benefactor, fundWalletKey, dust);

    const epoch1Amount = totalSent;

    console.log("Claim rewards for all initialized members");
    let snapshots = [];
    for (const [i, recipient] of daoMembers.entries()) {
      if (i >= 10) {
        break;
      }
      await claim(program, walletStateKey, fundWalletKey, recipient);
      let balance = await program.provider.connection.getBalance(
        recipient.publicKey,
        "confirmed"
      );
      snapshots.push(balance);
    }
    await claim(program, walletStateKey, fundWalletKey, creator);

    walletState = await program.account.walletState.fetch(
      walletStateKey,
      "confirmed"
    );
    assert.ok(walletState.dust.toNumber() == 0);
    assert.ok(walletState.lastSnapshot.toNumber() == 0);

    console.log("Added 50 bps to each Dao member");
    for (const [i, recipient] of daoMembers.entries()) {
      console.log("   ", i, recipient.publicKey.toBase58());
      if (i < 10) {
        await transferStake(
          program,
          walletStateKey,
          fundWalletKey,
          recipient,
          creator,
          creatorStateKey,
          50
        );
      } else {
        await createAndTransferStake(
          program,
          walletStateKey,
          fundWalletKey,
          recipient,
          creator,
          creatorStateKey,
          50
        );
      }
    }
    let amount = getRandomAmount(SOL1, SOL1);
    totalSent += amount;
    await donate(program, benefactor, fundWalletKey, amount);
    await claim(program, walletStateKey, fundWalletKey, creator);
    walletState = await program.account.walletState.fetch(
      walletStateKey,
      "confirmed"
    );

    console.log(
      "Fund balance: ",
      await program.provider.connection.getBalance(fundWalletKey, "confirmed")
    );
    console.log("Total Sent Without Dust: ", totalSent);
    console.log("Total Deposits", walletState.totalDeposits.toNumber());
    console.log("Dust", walletState.dust.toNumber());
    dust = walletState.totalShares.toNumber() - walletState.dust.toNumber();
    console.log("Amount to send", dust);
    totalSent += dust;
    await donate(program, benefactor, fundWalletKey, dust);

    console.log(
      "Fund balance: ",
      await program.provider.connection.getBalance(fundWalletKey, "confirmed")
    );
    console.log("Total Sent: ", totalSent);
    console.log("Epoch 1: ", epoch1Amount);

    console.log("Withdrawing for first 20 DAO members");
    for (const [i, recipient] of daoMembers.entries()) {
      if (i >= 20) {
        break;
      }
      await claim(program, walletStateKey, fundWalletKey, recipient);
    }
    console.log("Sending 1 more SOL");
    totalSent += SOL1;
    await donate(program, benefactor, fundWalletKey, SOL1);
    console.log("Withdrawing all remaining funds");
    for (const recipient of daoMembers) {
      await claim(program, walletStateKey, fundWalletKey, recipient);
    }
    await claim(program, walletStateKey, fundWalletKey, creator);

    walletState = await program.account.walletState.fetch(
      walletStateKey,
      "confirmed"
    );
    assert.ok(walletState.dust.toNumber() == 0);
    assert.ok(walletState.lastSnapshot.toNumber() == 0);
    console.log("Fund wallet has 0 remaining balance");
    const creatorState = await program.account.stake.fetch(creatorStateKey);
    console.log(
      `${creator.publicKey.toBase58()} (${creatorState.numShares.toNumber()}): `,
      await program.provider.connection.getBalance(creator.publicKey)
    );
    for (const [i, recipient] of daoMembers.entries()) {
      const recipientState = await program.account.stake.fetch(
        (
          await PublicKey.findProgramAddress(
            [walletStateKey.toBuffer(), recipient.publicKey.toBuffer()],
            program.programId
          )
        )[0]
      );
      let balance = await program.provider.connection.getBalance(
        recipient.publicKey,
        "confirmed"
      );
      console.log(
        `${recipient.publicKey.toBase58()} (${recipientState.numShares.toNumber()}): `,
        balance
      );
      if (i >= 10) {
        assert.ok(balance == (totalSent - epoch1Amount) / 200);
      } else {
        let bps = recipientState.numShares.toNumber();
        assert.ok(
          balance == ((totalSent - epoch1Amount) * bps) / 10000 + snapshots[i]
        );
      }
    }
    console.log("");
    console.log(
      "Fund balance: ",
      await program.provider.connection.getBalance(fundWalletKey, "confirmed")
    );
  });
});
