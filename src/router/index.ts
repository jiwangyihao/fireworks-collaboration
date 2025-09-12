import { createRouter, createWebHistory } from "vue-router";
import HomeView from "../views/HomeView.vue";
import CheckView from "../views/CheckView.vue";
import ProjectView from "../views/ProjectView.vue";
import HttpTester from "../views/HttpTester.vue";

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
  ],
});

export default router;
