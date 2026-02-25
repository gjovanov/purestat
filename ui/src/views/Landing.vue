<template>
  <div class="landing">
    <!-- Hero -->
    <v-container class="text-center py-16">
      <div class="d-flex justify-center mb-6">
        <img src="@/assets/logo.svg" alt="Purestat" height="64" />
      </div>
      <h1 class="text-h2 font-weight-bold mb-4">
        <span class="text-primary">Pure</span>, clean, honest stats
      </h1>
      <p class="text-h6 text-medium-emphasis mx-auto" style="max-width: 600px">
        Privacy-first, cookie-free web analytics. No consent banners needed.
        Lightweight script under 1KB. Open source.
      </p>
      <div class="mt-8 d-flex justify-center ga-4">
        <v-btn color="primary" size="large" to="/register">
          Get Started Free
        </v-btn>
        <v-btn variant="outlined" size="large" href="https://github.com/gjovanov/purestat" target="_blank">
          <v-icon start>mdi-github</v-icon>
          View on GitHub
        </v-btn>
      </div>
    </v-container>

    <!-- Features -->
    <v-container class="py-12">
      <v-row>
        <v-col cols="12" md="4" v-for="feature in features" :key="feature.title">
          <v-card class="pa-6 text-center h-100" variant="outlined">
            <v-icon size="48" :color="feature.color" class="mb-4">{{ feature.icon }}</v-icon>
            <h3 class="text-h6 font-weight-bold mb-2">{{ feature.title }}</h3>
            <p class="text-body-2 text-medium-emphasis">{{ feature.desc }}</p>
          </v-card>
        </v-col>
      </v-row>
    </v-container>

    <!-- Comparison -->
    <v-container class="py-12">
      <h2 class="text-h4 font-weight-bold text-center mb-8">Why Purestat?</h2>
      <v-row justify="center">
        <v-col cols="12" md="8">
          <v-table>
            <thead>
              <tr>
                <th>Feature</th>
                <th class="text-center text-primary">Purestat</th>
                <th class="text-center">Google Analytics</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="row in comparison" :key="row.feature">
                <td>{{ row.feature }}</td>
                <td class="text-center">
                  <v-icon :color="row.purestat ? 'secondary' : 'error'" size="20">
                    {{ row.purestat ? 'mdi-check-circle' : 'mdi-close-circle' }}
                  </v-icon>
                </td>
                <td class="text-center">
                  <v-icon :color="row.ga ? 'secondary' : 'error'" size="20">
                    {{ row.ga ? 'mdi-check-circle' : 'mdi-close-circle' }}
                  </v-icon>
                </td>
              </tr>
            </tbody>
          </v-table>
        </v-col>
      </v-row>
    </v-container>

    <!-- CTA -->
    <v-container class="text-center py-16">
      <h2 class="text-h4 font-weight-bold mb-4">Ready for honest analytics?</h2>
      <v-btn color="primary" size="large" to="/register">
        Start for Free
      </v-btn>
      <p class="text-body-2 text-medium-emphasis mt-4">
        No credit card required. Free plan includes 10,000 pageviews/month.
      </p>
    </v-container>

    <!-- Footer -->
    <v-footer class="text-center text-medium-emphasis pa-6">
      <v-container>
        <p class="text-body-2">
          Purestat &mdash; Open source, privacy-first analytics.
          MIT Licensed.
        </p>
      </v-container>
    </v-footer>
  </div>
</template>

<script setup lang="ts">
const features = [
  {
    icon: 'mdi-shield-check',
    title: 'Privacy First',
    desc: 'No cookies, no personal data collection. Compliant with GDPR, CCPA, and PECR out of the box.',
    color: 'secondary',
  },
  {
    icon: 'mdi-feather',
    title: 'Lightweight',
    desc: 'Under 1KB tracking script. Google Analytics is 73KB. Your site stays fast.',
    color: 'primary',
  },
  {
    icon: 'mdi-open-source-initiative',
    title: 'Open Source',
    desc: 'Fully open source under MIT license. Self-host or use our cloud. Audit every line of code.',
    color: 'accent',
  },
  {
    icon: 'mdi-rocket-launch',
    title: 'Built with Rust',
    desc: 'Axum + ClickHouse backend handles millions of events with minimal resources.',
    color: 'error',
  },
  {
    icon: 'mdi-chart-line',
    title: 'Beautiful Dashboard',
    desc: 'Clean, intuitive analytics dashboard. See visitors, sources, pages, and devices at a glance.',
    color: 'primary',
  },
  {
    icon: 'mdi-account-group',
    title: 'Team Friendly',
    desc: 'Multi-org support with roles and invites. Share dashboards with your team.',
    color: 'secondary',
  },
]

const comparison = [
  { feature: 'Cookie-free', purestat: true, ga: false },
  { feature: 'No consent banner needed', purestat: true, ga: false },
  { feature: 'Script size < 1KB', purestat: true, ga: false },
  { feature: 'Open source', purestat: true, ga: false },
  { feature: 'Self-hostable', purestat: true, ga: false },
  { feature: 'Real-time visitors', purestat: true, ga: true },
  { feature: 'Custom events', purestat: true, ga: true },
  { feature: 'Conversion tracking', purestat: true, ga: true },
  { feature: 'Free tier', purestat: true, ga: true },
]
</script>
