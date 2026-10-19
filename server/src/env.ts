export interface Env {
  DB: D1Database;
  /** The private signing key: 64 hex digits (the seed made by `lexo-pro-licence keygen`). A secret. */
  LICENCE_SIGNING_KEY: string;
  /** The public key that goes with it (the one built into the Pro pack). */
  LICENCE_PUBLIC_KEY: string;
  /** The id of the signing key (`kid` in the licence file); `k1` when not set. A new key gets a new id. */
  LICENCE_KID?: string;
  /** Older public keys that still count when a licence file is checked (JSON, {"k1": "<hex>"}); files signed by them are renewed with the current key. */
  LICENCE_OLD_PUBLIC_KEYS?: string;
  /** The signing secret of the webhook of the payment provider. A secret. */
  WEBHOOK_SECRET: string;
  /** A long random string; the admin page and its API need it. A secret. */
  ADMIN_TOKEN: string;
  /** The major version that new licences cover. */
  PRODUCT_MAJOR?: string;
  /** To mail the licence key to the buyer (Resend); without it the key is shown only in the admin page. */
  RESEND_API_KEY?: string;
  MAIL_FROM?: string;
  /** Where the replies of the buyers go (the Reply-To of the e-mail). */
  REPLY_TO?: string;
}
