import { createRouter, createWebHistory } from "vue-router";
import HomeView from "../views/HomeView.vue";
import CheckView from "../views/CheckView.vue";
import ProjectView from "../views/ProjectView.vue";
import HttpTester from "../views/developer-tools/HttpTester.vue";
import GitPanel from "../views/developer-tools/GitPanel.vue";
import CredentialView from "../views/developer-tools/CredentialView.vue";
import AuditLogView from "../views/developer-tools/AuditLogView.vue";
import WorkspaceView from "../views/developer-tools/WorkspaceView.vue";
import DeveloperToolsView from "../views/developer-tools/DeveloperToolsView.vue";

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: "/",
      name: "check",
      component: CheckView,
    },
    {
      path: "/project",
      name: "project",
      component: ProjectView,
    },
    {
      path: "/home",
      name: "home",
      component: HomeView,
    },
    {
      path: "/test",
      name: "github",
      component: () => import("../views/developer-tools/GitHubActionsView.vue"),
    },
    {
      path: "/http-tester",
      name: "httpTester",
      component: HttpTester,
    },
    {
      path: "/dev-tools",
      name: "devTools",
      component: DeveloperToolsView,
    },
    {
      path: "/git",
      name: "git",
      component: GitPanel,
    },
    {
      path: "/ip-pool",
      name: "ipPool",
      component: () => import("../views/developer-tools/IpPoolLab.vue"),
    },
    {
      path: "/credentials",
      name: "credentials",
      component: CredentialView,
    },
    {
      path: "/audit-logs",
      name: "auditLogs",
      component: AuditLogView,
    },
    {
      path: "/workspace",
      name: "workspace",
      component: WorkspaceView,
    },
    {
      path: "/observability",
      name: "observability",
      component: () => import("../views/developer-tools/ObservabilityView.vue"),
    },
    {
      path: "/document/:worktreePath(.*)",
      name: "document",
      component: () => import("../views/DocumentView.vue"),
      meta: { requiresProject: true },
    },
  ],
});

export default router;
