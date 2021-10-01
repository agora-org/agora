if (location.protocol === 'https:') {
  let elements = document.getElementsByClassName("clipboard-copy");

  for (let element of elements) {
    element.classList.add("enabled");
  }
}
