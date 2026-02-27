<template>
  <v-container class="py-12" style="max-width: 800px">
    <router-link to="/" class="text-body-2 text-primary text-decoration-none d-inline-flex align-center mb-8">
      <v-icon start size="18">mdi-arrow-left</v-icon>
      Back to home
    </router-link>

    <h1 class="text-h3 font-weight-bold mb-2">Privacy Policy</h1>
    <p class="text-body-2 text-medium-emphasis mb-8">Last updated: February 2026</p>

    <!-- Introduction -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Introduction</h2>
      <p class="text-body-1 mb-3">
        Purestat is a privacy-first, cookie-free web analytics platform. We are built from the ground up to provide
        website owners with useful analytics while fully respecting the privacy of their website visitors.
      </p>
      <p class="text-body-1">
        This privacy policy explains what data we collect, how we use it, and your rights regarding that data.
        We distinguish between two groups of people: <strong>account holders</strong> (users who sign up and use the
        Purestat dashboard) and <strong>website visitors</strong> (people who visit websites that use the Purestat
        tracking script).
      </p>
    </section>

    <!-- Privacy-first design -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Privacy-First Design</h2>
      <p class="text-body-1 mb-3">
        Purestat is designed to be privacy-first. Our tracking script does not use cookies, does not collect personal
        data, and does not track visitors across websites. Websites using Purestat do not need to display cookie
        consent banners or obtain visitor consent for analytics.
      </p>
      <v-alert type="info" variant="tonal" class="mb-3">
        <strong>No cookies. No personal data. No consent banners.</strong> Purestat is compliant with GDPR, CCPA,
        and PECR by design.
      </v-alert>
    </section>

    <!-- Visitor privacy -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Website Visitor Privacy</h2>
      <p class="text-body-1 mb-3">
        When someone visits a website that uses Purestat, we go to great lengths to ensure their privacy is protected:
      </p>
      <ul class="text-body-1 ml-6 mb-3">
        <li class="mb-2">
          <strong>IP addresses are never stored.</strong> We use HMAC-SHA256 hashing with a daily rotating salt to
          generate an anonymous visitor hash from the IP address, domain, and user agent. The original IP address is
          immediately discarded and never written to any database or log.
        </li>
        <li class="mb-2">
          <strong>The daily salt rotation</strong> ensures that the same visitor cannot be tracked across days. Each
          day a new salt is generated, making previous visitor hashes impossible to correlate with new ones.
        </li>
        <li class="mb-2">
          <strong>No cookies are set</strong> on the visitor's browser. There is no local storage, session storage,
          or fingerprinting of any kind.
        </li>
        <li class="mb-2">
          <strong>No cross-site tracking.</strong> Visitor hashes are domain-scoped, so a visitor on site A cannot be
          correlated with the same person on site B.
        </li>
      </ul>
    </section>

    <!-- Analytics data from visitors -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Analytics Data Collected from Website Visitors</h2>
      <p class="text-body-1 mb-3">
        The Purestat tracking script (under 1KB) collects the following anonymized, non-personal data points:
      </p>
      <ul class="text-body-1 ml-6 mb-3">
        <li class="mb-1">Page URL and page title</li>
        <li class="mb-1">Referrer URL (the page the visitor came from)</li>
        <li class="mb-1">Screen resolution</li>
        <li class="mb-1">Browser name and version (derived from user agent)</li>
        <li class="mb-1">Operating system (derived from user agent)</li>
        <li class="mb-1">Country (derived from IP at ingest time, IP is then discarded)</li>
        <li class="mb-1">Custom events (if configured by the site owner)</li>
      </ul>
      <p class="text-body-1">
        None of this data constitutes personally identifiable information (PII). It is impossible to identify an
        individual visitor from the data we collect.
      </p>
    </section>

    <!-- Account holder data -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Data Collected from Account Holders</h2>
      <p class="text-body-1 mb-3">
        When you create a Purestat account, we collect and store the following:
      </p>
      <ul class="text-body-1 ml-6 mb-3">
        <li class="mb-1"><strong>Account information:</strong> email address, username, display name</li>
        <li class="mb-1"><strong>Organization data:</strong> organization name, member roles, invitations</li>
        <li class="mb-1"><strong>Site configurations:</strong> domain names, site settings, goals</li>
        <li class="mb-1"><strong>Authentication data:</strong> password hash (argon2, never stored in plaintext)</li>
      </ul>
    </section>

    <!-- OAuth -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">OAuth Sign-In</h2>
      <p class="text-body-1 mb-3">
        When you use a third-party OAuth provider (Google, Facebook, GitHub, or LinkedIn) to sign in, we receive
        the following information from the provider:
      </p>
      <ul class="text-body-1 ml-6 mb-3">
        <li class="mb-1">Email address</li>
        <li class="mb-1">Display name</li>
        <li class="mb-1">Profile picture URL</li>
      </ul>
      <p class="text-body-1">
        We use this data solely to create and maintain your Purestat account. We do not access any other data from
        your third-party accounts.
      </p>
    </section>

    <!-- Billing -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Billing Data</h2>
      <p class="text-body-1">
        Payment processing is handled by <a href="https://stripe.com/privacy" target="_blank" class="text-primary">Stripe</a>.
        We do not store credit card numbers, CVVs, or full card details on our servers. We store only the Stripe
        customer and subscription identifiers needed to manage your plan.
      </p>
    </section>

    <!-- Cookies -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Cookies</h2>
      <p class="text-body-1 mb-3">
        <strong>For tracked websites:</strong> Purestat sets absolutely no cookies on the browsers of your website
        visitors. Our tracking script is entirely cookie-free.
      </p>
      <p class="text-body-1">
        <strong>For dashboard users:</strong> When you log in to the Purestat dashboard, we use a JWT authentication
        token stored as a cookie to maintain your session. This is a strictly necessary, first-party cookie required
        for the dashboard to function.
      </p>
    </section>

    <!-- GDPR / CCPA / PECR -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">GDPR, CCPA, and PECR Compliance</h2>
      <p class="text-body-1 mb-3">
        Purestat is designed to be compliant with privacy regulations by default:
      </p>
      <ul class="text-body-1 ml-6 mb-3">
        <li class="mb-2">
          <strong>GDPR (EU):</strong> We do not collect personal data from website visitors. No consent is required
          under GDPR for our analytics because we process no PII and set no cookies.
        </li>
        <li class="mb-2">
          <strong>CCPA (California):</strong> We do not sell personal information. Website visitors have no personal
          data stored with us to request deletion of.
        </li>
        <li class="mb-2">
          <strong>PECR (UK):</strong> Since we do not use cookies or similar tracking technologies on visitor
          browsers, PECR consent requirements do not apply to our tracking.
        </li>
      </ul>
      <p class="text-body-1">
        Websites using Purestat do not need to display a cookie consent banner for their analytics.
      </p>
    </section>

    <!-- Data retention -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Data Retention</h2>
      <p class="text-body-1 mb-3">
        Analytics data is retained for the duration of your account subscription. If you delete your account or a
        specific site, the associated analytics data is permanently deleted within 30 days.
      </p>
      <p class="text-body-1">
        Account holder data is retained as long as you maintain an active account. You may request deletion of your
        account and all associated data at any time.
      </p>
    </section>

    <!-- Security -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Security</h2>
      <p class="text-body-1">
        We take the security of your data seriously. All data is transmitted over HTTPS/TLS. Passwords are hashed
        using argon2. Our infrastructure is deployed on dedicated servers with strict access controls. Our codebase
        is open source and can be audited by anyone.
      </p>
    </section>

    <!-- Changes -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Changes to This Policy</h2>
      <p class="text-body-1">
        We may update this privacy policy from time to time. When we make significant changes, we will notify
        account holders by email or through the dashboard. We encourage you to review this page periodically.
      </p>
    </section>

    <!-- Contact -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">Contact</h2>
      <p class="text-body-1">
        If you have questions about this privacy policy or your data, contact us at
        <a href="mailto:privacy@purestat.ai" class="text-primary">privacy@purestat.ai</a>.
      </p>
    </section>
  </v-container>
</template>

<script setup lang="ts">
// Privacy Policy page — no logic needed
</script>

<style scoped>
a {
  text-decoration: underline;
}
</style>
