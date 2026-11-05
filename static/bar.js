for (const button of document.querySelectorAll('button')) {
  button.addEventListener('click', () => window.textActions.runAction(button.dataset.action));
}
