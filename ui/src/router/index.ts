import { createRouter, createWebHistory } from 'vue-router'
import { useAppStore } from '@/stores/app'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'landing',
      component: () => import('@/views/Landing.vue'),
      meta: { public: true, layout: 'blank' },
    },
    {
      path: '/login',
      name: 'login',
      component: () => import('@/views/Login.vue'),
      meta: { public: true, layout: 'blank' },
    },
    {
      path: '/register',
      name: 'register',
      component: () => import('@/views/Register.vue'),
      meta: { public: true, layout: 'blank' },
    },
    {
      path: '/invite/:code',
      name: 'invite-accept',
      component: () => import('@/views/InviteAccept.vue'),
    },
    {
      path: '/orgs',
      name: 'orgs',
      component: () => import('@/views/OrgList.vue'),
    },
    {
      path: '/org/:orgId/sites',
      name: 'sites',
      component: () => import('@/views/Sites.vue'),
    },
    {
      path: '/org/:orgId/site/:siteId',
      name: 'dashboard',
      component: () => import('@/views/Dashboard.vue'),
    },
    {
      path: '/org/:orgId/site/:siteId/goals',
      name: 'goals',
      component: () => import('@/views/Goals.vue'),
    },
    {
      path: '/org/:orgId/site/:siteId/settings',
      name: 'site-settings',
      component: () => import('@/views/SiteSettings.vue'),
    },
    {
      path: '/org/:orgId/site/:siteId/api-keys',
      name: 'api-keys',
      component: () => import('@/views/ApiKeys.vue'),
    },
    {
      path: '/org/:orgId/members',
      name: 'members',
      component: () => import('@/views/Members.vue'),
    },
    {
      path: '/org/:orgId/billing',
      name: 'billing',
      component: () => import('@/views/Billing.vue'),
    },
    {
      path: '/org/:orgId/settings',
      name: 'org-settings',
      component: () => import('@/views/OrgSettings.vue'),
    },
    {
      path: '/settings',
      name: 'user-settings',
      component: () => import('@/views/UserSettings.vue'),
    },
  ],
})

router.beforeEach((to) => {
  const appStore = useAppStore()
  if (!to.meta.public && !appStore.isAuthenticated) {
    return { name: 'login', query: { redirect: to.fullPath } }
  }
})

export default router
