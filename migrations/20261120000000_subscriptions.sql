-- Subscriptions paid through Stripe (src/stripe.rs, src/routes/billing.rs).
--
-- Stripe is the source of truth: these rows are a copy, refreshed from its
-- API on every webhook call. The `abonne` role is what the rest of the site
-- reads; it follows the subscriptions, and an admin can also give it by hand
-- from the roles page (an offered subscription).

INSERT INTO site.roles (slug, nom, categorie, couleur, niveau, description, ordre)
VALUES (
    'abonne',
    'Abonné',
    'acces',
    '#d9a21b',
    0,
    'Abonnement Pulsars en cours (payé ou offert).',
    5
)
ON CONFLICT (slug) DO NOTHING;

-- The Stripe customer of a member: created at their first checkout.
CREATE TABLE site.stripe_customers (
    user_id bigint PRIMARY KEY REFERENCES site.users(id) ON DELETE CASCADE,
    customer_id varchar(255) NOT NULL UNIQUE,
    created_at timestamp(0) without time zone NOT NULL DEFAULT (date_trunc('second', now() AT TIME ZONE 'UTC'))
);

-- Each Checkout session opened, with the moment the member asked for
-- immediate access and waived the withdrawal period (French consumer law,
-- art. L221-28 of the Code de la consommation): the proof of that consent.
CREATE TABLE site.checkout_sessions (
    id varchar(255) PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES site.users(id) ON DELETE CASCADE,
    price_id varchar(255) NOT NULL,
    withdrawal_waived_at timestamp(0) without time zone NOT NULL,
    completed_at timestamp(0) without time zone,
    created_at timestamp(0) without time zone NOT NULL DEFAULT (date_trunc('second', now() AT TIME ZONE 'UTC'))
);

CREATE INDEX checkout_sessions_user_index ON site.checkout_sessions (user_id);

-- The member's subscriptions, as Stripe last described them.
CREATE TABLE site.subscriptions (
    id varchar(255) PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES site.users(id) ON DELETE CASCADE,
    status varchar(32) NOT NULL,
    price_id varchar(255),
    current_period_end timestamp(0) without time zone,
    cancel_at_period_end boolean NOT NULL DEFAULT false,
    created_at timestamp(0) without time zone NOT NULL DEFAULT (date_trunc('second', now() AT TIME ZONE 'UTC')),
    updated_at timestamp(0) without time zone NOT NULL DEFAULT (date_trunc('second', now() AT TIME ZONE 'UTC'))
);

CREATE INDEX subscriptions_user_index ON site.subscriptions (user_id);
