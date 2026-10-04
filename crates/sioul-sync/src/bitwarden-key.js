// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A security key asked for Bitwarden, from a page of Bitwarden's own vault
// (the one address Bitwarden takes keys' signatures from), held unseen in
// Sioul's dialog: Sioul calls `window.sioulKey()` until it answers; the first
// call asks the key. For a second step ("factor"), the answer is the step's
// code, as Bitwarden's own security-key page gives it. For the key alone
// ("passkey"), the passkey and its secret (WebAuthn PRF, salted as
// Bitwarden's apps salt it). The answer waits in this page until taken, and
// is never put in an address, which would carry it out. Called by
// `key_script` with the mode, the options Bitwarden gave, and the salt
// (base64url).

((mode, options, salt) => {
    const bytes = text => Uint8Array.from(atob(text.replace(/-/g, "+").replace(/_/g, "/")), c => c.charCodeAt(0));
    const base64url = buffer => btoa(String.fromCharCode(...new Uint8Array(buffer))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    let asked = false;
    let answer = "";

    // Not used in time, or cancelled: one sentence for both in Sioul's words.
    // Anything else, options that cannot be read included, is said too: the
    // dialog never waits for an answer that cannot come.
    function fail(error) {
        const name = error && error.name;
        answer = JSON.stringify({ error: name === "NotAllowedError" ? "not-allowed" : String((error && error.message) || error) });
    }

    function ask() {
        // Bitwarden's options as they are (its own page passes them so), the bytes made bytes.
        const publicKey = Object.assign({}, options, {
            challenge: bytes(options.challenge),
            allowCredentials: (options.allowCredentials || []).map(c => Object.assign({}, c, { id: bytes(c.id) }))
        });
        if (mode === "passkey")
            publicKey.extensions = Object.assign({}, publicKey.extensions, { prf: { eval: { first: bytes(salt) } } });
        navigator.credentials.get({ publicKey: publicKey }).then(credential => {
            const response = credential.response;
            if (mode === "passkey") {
                const prf = (credential.getClientExtensionResults().prf || {}).results || {};
                answer = JSON.stringify({
                    // As Bitwarden's apps send it: no extension results, so the secret stays here.
                    response: {
                        id: credential.id,
                        rawId: base64url(credential.rawId),
                        type: credential.type,
                        extensions: {},
                        response: {
                            authenticatorData: base64url(response.authenticatorData),
                            signature: base64url(response.signature),
                            clientDataJSON: base64url(response.clientDataJSON),
                            userHandle: response.userHandle ? base64url(response.userHandle) : ""
                        }
                    },
                    secret: prf.first ? base64url(prf.first) : ""
                });
            } else {
                // The step's code, word for word as Bitwarden's security-key page writes it.
                answer = JSON.stringify({
                    token: JSON.stringify({
                        id: credential.id,
                        rawId: base64url(credential.rawId),
                        type: credential.type,
                        extensions: credential.getClientExtensionResults(),
                        response: {
                            authenticatorData: base64url(response.authenticatorData),
                            clientDataJson: base64url(response.clientDataJSON),
                            signature: base64url(response.signature)
                        }
                    })
                });
            }
        }).catch(fail);
    }

    // Sioul's question: the key asked the first time; then the answer once, "" until it comes.
    window.sioulKey = () => {
        if (!asked) {
            asked = true;
            try {
                ask();
            } catch (error) {
                fail(error);
            }
        }
        const taken = answer;
        answer = "";
        return taken;
    };
})
