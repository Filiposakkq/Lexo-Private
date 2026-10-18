export interface Env {
  DB: D1Database;
  /** The pages of the website (the files of web/). */
  ASSETS: Fetcher;
  /** Optional emergency password for /admin (at least 16 characters). A secret; not set = only Cloudflare Access lets you in. */
  ADMIN_TOKEN?: string;
  /** The team domain of Cloudflare Access, e.g. "aside.cloudflareaccess.com". */
  ACCESS_TEAM_DOMAIN?: string;
  /** The Application Audience (AUD) tag of the Access application that protects /admin. */
  ACCESS_AUD?: string;
  /** Only for tests: the public keys of Access as JSON, instead of fetching them. */
  ACCESS_JWKS?: string;
  /** A long random string mixed into the hashes of addresses and browser ids. A secret; set once, never change it. */
  HASH_SALT?: string;
  /** A Resend API key that may send as MAIL_FROM. A secret; without it nothing is mailed (everything is still saved). */
  RESEND_API_KEY?: string;
  /** The secret key of the Turnstile widget. A secret; without it the forms are not checked for bots. */
  TURNSTILE_SECRET?: string;
  /** The public site key of the Turnstile widget, handed to the pages. */
  TURNSTILE_SITE_KEY?: string;
  SITE_URL?: string;
  MAIL_FROM?: string;
  OWNER_EMAIL?: string;
}
