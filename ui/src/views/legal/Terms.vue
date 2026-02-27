<template>
  <v-container class="py-12" style="max-width: 800px">
    <router-link to="/" class="text-body-2 text-primary text-decoration-none d-inline-flex align-center mb-8">
      <v-icon start size="18">mdi-arrow-left</v-icon>
      Back to home
    </router-link>

    <h1 class="text-h3 font-weight-bold mb-2">Terms of Service</h1>
    <p class="text-body-2 text-medium-emphasis mb-8">Last updated: February 2026</p>

    <!-- Introduction -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">1. Introduction</h2>
      <p class="text-body-1 mb-3">
        These Terms of Service ("Terms") govern your use of Purestat ("Service"), a privacy-first, cookie-free web
        analytics platform operated at <a href="https://purestat.ai" class="text-primary">purestat.ai</a>.
      </p>
      <p class="text-body-1">
        By creating an account or using the Service, you agree to be bound by these Terms. If you do not agree,
        do not use the Service.
      </p>
    </section>

    <!-- Service description -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">2. Service Description</h2>
      <p class="text-body-1">
        Purestat provides privacy-first web analytics. You add a lightweight tracking script (under 1KB) to your
        website, and Purestat collects anonymized, cookie-free analytics data. You access your analytics through
        the Purestat dashboard. The service is designed to be fully compliant with GDPR, CCPA, and PECR without
        requiring consent banners on your website.
      </p>
    </section>

    <!-- Eligibility -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">3. Eligibility</h2>
      <p class="text-body-1">
        You must be at least 16 years old to use the Service. By using Purestat, you represent that you meet this
        age requirement and have the legal capacity to enter into these Terms.
      </p>
    </section>

    <!-- Account responsibilities -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">4. Account Responsibilities</h2>
      <p class="text-body-1 mb-3">
        You are responsible for:
      </p>
      <ul class="text-body-1 ml-6 mb-3">
        <li class="mb-1">Maintaining the security of your account credentials</li>
        <li class="mb-1">All activity that occurs under your account</li>
        <li class="mb-1">Providing accurate and current account information</li>
        <li class="mb-1">Notifying us promptly of any unauthorized access</li>
      </ul>
      <p class="text-body-1">
        You may not share your account with others. Use the organization and team features to grant access to
        colleagues.
      </p>
    </section>

    <!-- Acceptable use -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">5. Acceptable Use</h2>
      <p class="text-body-1 mb-3">
        You agree not to:
      </p>
      <ul class="text-body-1 ml-6 mb-3">
        <li class="mb-1">Use the Service for any unlawful purpose</li>
        <li class="mb-1">Attempt to reverse-engineer, tamper with, or circumvent any security measures</li>
        <li class="mb-1">Send artificial or bot-generated traffic to inflate analytics</li>
        <li class="mb-1">Use the Service to track websites you do not own or have authorization to monitor</li>
        <li class="mb-1">Resell or redistribute the Service without prior written consent</li>
        <li class="mb-1">Interfere with or disrupt the Service infrastructure</li>
      </ul>
    </section>

    <!-- Fair usage -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">6. Plan Tiers and Fair Usage</h2>
      <p class="text-body-1 mb-3">
        Purestat offers the following plan tiers with monthly pageview limits:
      </p>
      <v-table class="mb-3">
        <thead>
          <tr>
            <th>Plan</th>
            <th class="text-center">Monthly Pageviews</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>Free</td>
            <td class="text-center">10,000</td>
          </tr>
          <tr>
            <td>Pro</td>
            <td class="text-center">100,000</td>
          </tr>
          <tr>
            <td>Business</td>
            <td class="text-center">1,000,000</td>
          </tr>
        </tbody>
      </v-table>
      <p class="text-body-1">
        If you exceed your plan's pageview limit, we will notify you and may ask you to upgrade. We will not
        immediately stop collecting data, but sustained overuse may result in temporary data collection pauses
        until the next billing cycle or an upgrade.
      </p>
    </section>

    <!-- Intellectual property -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">7. Intellectual Property</h2>
      <p class="text-body-1 mb-3">
        The Purestat source code is released under the MIT License. You are free to use, modify, and distribute
        the code in accordance with that license.
      </p>
      <p class="text-body-1">
        The Purestat name, logo, and branding are trademarks. The MIT license applies to the source code only and
        does not grant rights to use Purestat trademarks.
      </p>
    </section>

    <!-- Tracker script -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">8. Tracker Script Usage</h2>
      <p class="text-body-1">
        You may embed the Purestat tracking script on any website you own or are authorized to manage. The script
        is served from our infrastructure and must not be modified in ways that alter its privacy-preserving behavior.
        You may self-host the tracker if you use the open-source version, in which case you are responsible for
        maintaining its configuration.
      </p>
    </section>

    <!-- Data ownership -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">9. Data Ownership</h2>
      <p class="text-body-1 mb-3">
        <strong>You own your analytics data.</strong> All analytics data collected from your websites belongs to you.
        We do not sell, share, or monetize your analytics data in any way.
      </p>
      <p class="text-body-1">
        You may export your data at any time through the dashboard or API. Upon account deletion, your data will be
        permanently removed within 30 days.
      </p>
    </section>

    <!-- Open source -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">10. Open Source License</h2>
      <p class="text-body-1">
        Purestat is open source software released under the
        <a href="https://opensource.org/licenses/MIT" target="_blank" class="text-primary">MIT License</a>.
        The source code is available at
        <a href="https://github.com/gjovanov/purestat" target="_blank" class="text-primary">github.com/gjovanov/purestat</a>.
        You are free to self-host Purestat under the terms of the MIT License.
      </p>
    </section>

    <!-- Billing -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">11. Billing and Payments</h2>
      <p class="text-body-1 mb-3">
        Paid plans are billed on a monthly or annual subscription basis through
        <a href="https://stripe.com" target="_blank" class="text-primary">Stripe</a>.
        All prices are listed in USD unless otherwise stated.
      </p>
      <ul class="text-body-1 ml-6 mb-3">
        <li class="mb-1">Subscriptions renew automatically unless cancelled before the renewal date</li>
        <li class="mb-1">You may upgrade, downgrade, or cancel your plan at any time from the dashboard</li>
        <li class="mb-1">Downgrades and cancellations take effect at the end of the current billing period</li>
        <li class="mb-1">Refunds are handled on a case-by-case basis; contact us if you believe you are owed a refund</li>
      </ul>
    </section>

    <!-- Termination -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">12. Termination</h2>
      <p class="text-body-1 mb-3">
        You may terminate your account at any time by deleting it from the dashboard or by contacting us.
      </p>
      <p class="text-body-1">
        We reserve the right to suspend or terminate accounts that violate these Terms, engage in abusive behavior,
        or consistently exceed fair usage limits without upgrading. We will provide reasonable notice before
        termination except in cases of serious violations.
      </p>
    </section>

    <!-- Disclaimers -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">13. Disclaimers</h2>
      <p class="text-body-1">
        The Service is provided "as is" and "as available" without warranties of any kind, whether express or
        implied, including but not limited to implied warranties of merchantability, fitness for a particular
        purpose, and non-infringement. We do not warrant that the Service will be uninterrupted, error-free, or
        completely secure.
      </p>
    </section>

    <!-- Limitation of liability -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">14. Limitation of Liability</h2>
      <p class="text-body-1">
        To the maximum extent permitted by law, Purestat and its operators shall not be liable for any indirect,
        incidental, special, consequential, or punitive damages, or any loss of profits or revenues, whether
        incurred directly or indirectly, or any loss of data, use, or goodwill. Our total aggregate liability for
        any claim arising out of or relating to the Service shall not exceed the amount you paid us in the twelve
        (12) months preceding the claim.
      </p>
    </section>

    <!-- Governing law -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">15. Governing Law</h2>
      <p class="text-body-1">
        These Terms shall be governed by and construed in accordance with the laws of the European Union and the
        applicable member state jurisdiction. Any disputes arising from these Terms shall be resolved in the
        competent courts of that jurisdiction.
      </p>
    </section>

    <!-- Changes -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">16. Changes to These Terms</h2>
      <p class="text-body-1">
        We may update these Terms from time to time. When we make significant changes, we will notify account
        holders by email or through the dashboard at least 30 days before the changes take effect. Continued use
        of the Service after changes become effective constitutes acceptance of the revised Terms.
      </p>
    </section>

    <!-- Contact -->
    <section class="mb-8">
      <h2 class="text-h5 font-weight-bold mb-3">17. Contact</h2>
      <p class="text-body-1">
        If you have questions about these Terms, contact us at
        <a href="mailto:legal@purestat.ai" class="text-primary">legal@purestat.ai</a>.
      </p>
    </section>
  </v-container>
</template>

<script setup lang="ts">
// Terms of Service page — no logic needed
</script>

<style scoped>
a {
  text-decoration: underline;
}
</style>
