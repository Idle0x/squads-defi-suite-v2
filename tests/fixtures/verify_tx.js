// Solana wire-format verification script
// Verifies that base64 output from squads-defi-core decodes correctly.
//
// Usage: node tests/fixtures/verify_tx.js <base64_tx>
// Install: npm install @solana/web3.js
//
// This script proves the plugin output is real Solana wire format,
// not borsh-serialized with u32 length prefixes.

const { VersionedTransaction } = require('@solana/web3.js');

function verifyTx(base64String) {
    try {
        const buffer = Buffer.from(base64String, 'base64');
        const tx = VersionedTransaction.deserialize(buffer);
        
        console.log('✅ Transaction decoded successfully!');
        console.log('  Account keys:', tx.message.staticAccountKeys.length);
        console.log('  Instructions:', tx.message.compiledInstructions.length);
        console.log('  Signatures required:', tx.message.header.numRequiredSignatures);
        
        tx.message.staticAccountKeys.forEach((key, i) => {
            console.log(`  Account[${i}]: ${key.toBase58()}`);
        });
        
        tx.message.compiledInstructions.forEach((ix, i) => {
            console.log(`  Instruction[${i}]: program=${ix.programIdIndex}, accounts=${ix.accountKeyIndexes.length}, data=${ix.data.length} bytes`);
        });
        
        console.log('\n✅ VERIFIED: Valid Solana wire format.');
        return true;
    } catch (e) {
        console.error('❌ FAILED:', e.message);
        return false;
    }
}

const args = process.argv.slice(2);
if (args.length === 0) {
    console.log('Usage: node verify_tx.js <base64_tx>');
    process.exit(1);
}
process.exit(verifyTx(args[0]) ? 0 : 1);
