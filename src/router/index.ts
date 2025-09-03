import { createRouter, createWebHistory } from "vue-router";
import HomeView from "../views/HomeView.vue";
import CheckView from "../views/CheckView.vue";

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: "/",
      name: "check",
      component: CheckView,
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
  ],
});

export default router;
