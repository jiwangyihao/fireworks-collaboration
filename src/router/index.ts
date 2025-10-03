import { createRouter, createWebHistory } from "vue-router";
import HomeView from "../views/HomeView.vue";
import CheckView from "../views/CheckView.vue";
import ProjectView from "../views/ProjectView.vue";
import HttpTester from "../views/HttpTester.vue";
import GitPanel from "../views/GitPanel.vue";
import CredentialView from "../views/CredentialView.vue";
import AuditLogView from "../views/AuditLogView.vue";

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
      component: () => import("../views/GitHubActionsView.vue"),
    },
    {
      path: "/http-tester",
      name: "httpTester",
      component: HttpTester,
    },
    {
      path: "/git",
      name: "git",
      component: GitPanel,
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
  ],
});

export default router;
