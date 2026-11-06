for (const button of document.querySelectorAll('button')) {
  button.addEventListener('click', () => window.fluent.runAction(button.dataset.action));
}
