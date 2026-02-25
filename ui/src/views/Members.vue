<template>
  <div>
    <div class="d-flex align-center mb-6">
      <h1 class="text-h5 font-weight-bold">{{ $t('nav.members') }}</h1>
      <v-spacer />
      <v-btn color="primary" prepend-icon="mdi-account-plus" @click="showInviteDialog = true">
        Invite
      </v-btn>
    </div>

    <!-- Members List -->
    <v-card class="mb-6">
      <v-card-title class="text-subtitle-1 font-weight-bold">Members</v-card-title>
      <v-list>
        <v-list-item v-for="member in members" :key="member.id">
          <template v-slot:prepend>
            <v-avatar color="primary" size="36">
              <span class="text-white text-body-2">{{ member.display_name?.charAt(0).toUpperCase() || '?' }}</span>
            </v-avatar>
          </template>
          <v-list-item-title>{{ member.display_name || member.email }}</v-list-item-title>
          <v-list-item-subtitle>{{ member.email }} &middot; {{ member.role }}</v-list-item-subtitle>
          <template v-slot:append>
            <v-select
              v-if="member.role !== 'owner'"
              :model-value="member.role"
              :items="roles"
              density="compact"
              variant="outlined"
              hide-details
              style="max-width: 120px"
              class="mr-2"
              @update:model-value="handleRoleChange(member.id, $event)"
            />
            <v-btn
              v-if="member.role !== 'owner'"
              icon
              size="small"
              color="error"
              variant="text"
              @click="handleRemove(member.id)"
            >
              <v-icon>mdi-account-remove</v-icon>
            </v-btn>
          </template>
        </v-list-item>
      </v-list>
    </v-card>

    <!-- Pending Invites -->
    <v-card v-if="invites.length > 0">
      <v-card-title class="text-subtitle-1 font-weight-bold">Pending Invites</v-card-title>
      <v-list>
        <v-list-item v-for="invite in invites" :key="invite.id">
          <template v-slot:prepend>
            <v-icon color="accent">mdi-email-outline</v-icon>
          </template>
          <v-list-item-title>{{ invite.target_email || 'Open invite' }}</v-list-item-title>
          <v-list-item-subtitle>
            Code: {{ invite.code }} &middot; Uses: {{ invite.use_count }}/{{ invite.max_uses }}
          </v-list-item-subtitle>
          <template v-slot:append>
            <v-btn icon size="small" variant="text" @click="copyInviteLink(invite.code)">
              <v-icon>mdi-content-copy</v-icon>
            </v-btn>
            <v-btn icon size="small" color="error" variant="text" @click="handleRevokeInvite(invite.id)">
              <v-icon>mdi-close</v-icon>
            </v-btn>
          </template>
        </v-list-item>
      </v-list>
    </v-card>

    <!-- Invite Dialog -->
    <v-dialog v-model="showInviteDialog" max-width="450">
      <v-card class="pa-6">
        <v-card-title class="text-h6 pa-0 mb-4">Invite to Organization</v-card-title>
        <v-form @submit.prevent="handleInvite">
          <v-text-field
            v-model="inviteEmail"
            label="Email (optional)"
            placeholder="teammate@company.com"
            class="mb-2"
          />
          <v-select
            v-model="inviteRole"
            :items="roles"
            label="Role"
            class="mb-2"
          />
          <v-text-field
            v-model.number="inviteMaxUses"
            label="Max uses"
            type="number"
            min="1"
            class="mb-4"
          />
          <div class="d-flex justify-end ga-2">
            <v-btn variant="text" @click="showInviteDialog = false">{{ $t('common.cancel') }}</v-btn>
            <v-btn type="submit" color="primary" :loading="inviting">Create Invite</v-btn>
          </div>
        </v-form>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useHttpClient } from '@/composables/useHttpClient'

interface Member {
  id: string
  user_id: string
  email: string
  display_name: string
  role: string
}

interface Invite {
  id: string
  code: string
  target_email: string | null
  max_uses: number
  use_count: number
  status: string
}

const route = useRoute()
const orgId = route.params.orgId as string

const members = ref<Member[]>([])
const invites = ref<Invite[]>([])
const showInviteDialog = ref(false)
const inviteEmail = ref('')
const inviteRole = ref('viewer')
const inviteMaxUses = ref(1)
const inviting = ref(false)

const roles = ['admin', 'viewer']

onMounted(() => {
  fetchMembers()
  fetchInvites()
})

async function fetchMembers() {
  const { get } = useHttpClient()
  members.value = await get<Member[]>(`/org/${orgId}/member`)
}

async function fetchInvites() {
  const { get } = useHttpClient()
  invites.value = await get<Invite[]>(`/org/${orgId}/invite`)
}

async function handleRoleChange(memberId: string, role: string) {
  const { put } = useHttpClient()
  await put(`/org/${orgId}/member/${memberId}`, { role })
  const m = members.value.find((m) => m.id === memberId)
  if (m) m.role = role
}

async function handleRemove(memberId: string) {
  if (confirm('Remove this member from the organization?')) {
    const { del } = useHttpClient()
    await del(`/org/${orgId}/member/${memberId}`)
    members.value = members.value.filter((m) => m.id !== memberId)
  }
}

async function handleInvite() {
  inviting.value = true
  try {
    const { post } = useHttpClient()
    const invite = await post<Invite>(`/org/${orgId}/invite`, {
      target_email: inviteEmail.value || undefined,
      role: inviteRole.value,
      max_uses: inviteMaxUses.value,
    })
    invites.value.push(invite)
    showInviteDialog.value = false
    inviteEmail.value = ''
    inviteRole.value = 'viewer'
    inviteMaxUses.value = 1
  } finally {
    inviting.value = false
  }
}

async function handleRevokeInvite(inviteId: string) {
  if (confirm('Revoke this invite?')) {
    const { del } = useHttpClient()
    await del(`/org/${orgId}/invite/${inviteId}`)
    invites.value = invites.value.filter((i) => i.id !== inviteId)
  }
}

function copyInviteLink(code: string) {
  navigator.clipboard.writeText(`${window.location.origin}/invite/${code}`)
}
</script>
