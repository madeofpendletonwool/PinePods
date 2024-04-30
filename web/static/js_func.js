// // Truncate or expand based on the element's max-height
// function toggleDescription(guid, expanded) {
//     const content = document.querySelector(`.desc-${guid}`);
//     if (expanded) {
//         content.style.maxHeight = "none";  // Expand fully
//     } else {
//         content.style.maxHeight = "100px"; // Truncate (set to desired truncation height)
//     }
// }

document.addEventListener('DOMContentLoaded', () => {
    const allDescriptions = document.querySelectorAll('[id^="desc-"]');
    allDescriptions.forEach(desc => {
        const id = desc.id.replace("desc-", "");
        toggleDescription(id, false); // Start all as collapsed
    });
});


window.toggleDescription = function(guid, shouldExpand) {
    const descContainer = document.querySelector(`.desc-${guid}`);
    if (descContainer) {
        if (shouldExpand) {
            descContainer.classList.add('expanded');
            descContainer.classList.remove('collapsed');
        } else {
            descContainer.classList.remove('expanded');
            descContainer.classList.add('collapsed');
        }
    } else {
        console.error("Description container not found for GUID: " + guid);
    }
}
