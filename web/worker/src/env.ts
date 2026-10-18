export interface Env {
  DB: D1Database;
  /** The pages of the website (the files of web/). */
  ASSETS: Fetcher;
  /** A long random string for /admin and its API. A secret. */
  ADMIN_TOKEN: string;
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
